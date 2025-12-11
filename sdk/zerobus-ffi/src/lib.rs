use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

use databricks_zerobus_ingest_sdk::{
    ZerobusSdk, ZerobusStream, ZerobusError,
    TableProperties, StreamConfigurationOptions, RecordPayload,
};
use databricks_zerobus_ingest_sdk::databricks::zerobus::RecordType;
use prost::Message;

// Global Tokio runtime for handling async Rust calls
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

// Opaque types for Go
#[repr(C)]
pub struct CZerobusSdk {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CZerobusStream {
    _private: [u8; 0],
}

// Result type for FFI calls
#[repr(C)]
pub struct CResult {
    pub success: bool,
    pub error_message: *mut c_char,
    pub is_retryable: bool,
}

impl CResult {
    fn success() -> Self {
        CResult {
            success: true,
            error_message: ptr::null_mut(),
            is_retryable: false,
        }
    }

    fn error(err: ZerobusError) -> Self {
        let is_retryable = err.is_retryable();
        let message = CString::new(err.to_string())
            .unwrap_or_else(|_| CString::new("Unknown error").unwrap());

        CResult {
            success: false,
            error_message: message.into_raw(),
            is_retryable,
        }
    }
}

// Configuration options
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CStreamConfigurationOptions {
    pub max_inflight_records: usize,
    pub recovery: bool,
    pub recovery_timeout_ms: u64,
    pub recovery_backoff_ms: u64,
    pub recovery_retries: u32,
    pub server_lack_of_ack_timeout_ms: u64,
    pub flush_timeout_ms: u64,
    pub record_type: i32,
}

impl From<CStreamConfigurationOptions> for StreamConfigurationOptions {
    fn from(c_opts: CStreamConfigurationOptions) -> Self {
        let mut opts = StreamConfigurationOptions::default();
        opts.max_inflight_records = c_opts.max_inflight_records;
        opts.recovery = c_opts.recovery;
        opts.recovery_timeout_ms = c_opts.recovery_timeout_ms;
        opts.recovery_backoff_ms = c_opts.recovery_backoff_ms;
        opts.recovery_retries = c_opts.recovery_retries;
        opts.server_lack_of_ack_timeout_ms = c_opts.server_lack_of_ack_timeout_ms;
        opts.flush_timeout_ms = c_opts.flush_timeout_ms;
        opts.record_type = match c_opts.record_type {
            1 => RecordType::Proto,
            2 => RecordType::Json,
            _ => RecordType::Unspecified,
        };
        opts
    }
}

// Helper to convert C string to Rust String
unsafe fn c_str_to_string(c_str: *const c_char) -> Result<String, &'static str> {
    if c_str.is_null() {
        return Err("Null pointer passed");
    }
    CStr::from_ptr(c_str)
        .to_str()
        .map(|s| s.to_string())
        .map_err(|_| "Invalid UTF-8 string")
}

// ============================================================================
// SDK Functions
// ============================================================================

/// Create a new ZerobusSdk instance
/// Returns NULL on error. Check the result parameter for error details.
#[no_mangle]
pub extern "C" fn zerobus_sdk_new(
    zerobus_endpoint: *const c_char,
    unity_catalog_url: *const c_char,
    result: *mut CResult,
) -> *mut CZerobusSdk {
    let res = (|| -> Result<*mut CZerobusSdk, String> {
        let endpoint = unsafe { c_str_to_string(zerobus_endpoint).map_err(|e| e.to_string())? };
        let catalog_url = unsafe { c_str_to_string(unity_catalog_url).map_err(|e| e.to_string())? };

        let sdk = ZerobusSdk::new(endpoint, catalog_url).map_err(|e| e.to_string())?;
        let boxed = Box::new(sdk);
        Ok(Box::into_raw(boxed) as *mut CZerobusSdk)
    })();

    match res {
        Ok(sdk_ptr) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            sdk_ptr
        }
        Err(err) => {
            if !result.is_null() {
                let err_msg = CString::new(err).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
                unsafe {
                    *result = CResult {
                        success: false,
                        error_message: err_msg.into_raw(),
                        is_retryable: false,
                    };
                }
            }
            ptr::null_mut()
        }
    }
}

/// Free the SDK instance
#[no_mangle]
pub extern "C" fn zerobus_sdk_free(sdk: *mut CZerobusSdk) {
    if !sdk.is_null() {
        unsafe {
            let _ = Box::from_raw(sdk as *mut ZerobusSdk);
        }
    }
}

/// Create a stream with OAuth authentication
/// descriptor_proto_bytes: protobuf-encoded DescriptorProto (can be NULL for JSON streams)
#[no_mangle]
pub extern "C" fn zerobus_sdk_create_stream(
    sdk: *mut CZerobusSdk,
    table_name: *const c_char,
    descriptor_proto_bytes: *const u8,
    descriptor_proto_len: usize,
    client_id: *const c_char,
    client_secret: *const c_char,
    options: *const CStreamConfigurationOptions,
    result: *mut CResult,
) -> *mut CZerobusStream {
    if sdk.is_null() {
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("SDK pointer is null").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        return ptr::null_mut();
    }

    let sdk_ref = unsafe { &*(sdk as *const ZerobusSdk) };

    let res = RUNTIME.block_on(async {
        let table_name_str = unsafe { c_str_to_string(table_name).map_err(|e| e.to_string())? };
        let client_id_str = unsafe { c_str_to_string(client_id).map_err(|e| e.to_string())? };
        let client_secret_str = unsafe { c_str_to_string(client_secret).map_err(|e| e.to_string())? };

        // Decode descriptor if provided
        let descriptor_proto = if !descriptor_proto_bytes.is_null() && descriptor_proto_len > 0 {
            let bytes = unsafe { std::slice::from_raw_parts(descriptor_proto_bytes, descriptor_proto_len) };
            Some(prost_types::DescriptorProto::decode(bytes).map_err(|e| e.to_string())?)
        } else {
            None
        };

        let table_props = TableProperties {
            table_name: table_name_str,
            descriptor_proto,
        };

        let stream_options = if !options.is_null() {
            Some(unsafe { (*options).into() })
        } else {
            None
        };

        let stream = sdk_ref
            .create_stream(table_props, client_id_str, client_secret_str, stream_options)
            .await
            .map_err(|e| e.to_string())?;

        let boxed = Box::new(stream);
        Ok::<*mut CZerobusStream, String>(Box::into_raw(boxed) as *mut CZerobusStream)
    });

    match res {
        Ok(stream_ptr) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            stream_ptr
        }
        Err(err) => {
            if !result.is_null() {
                let err_msg = CString::new(err).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
                unsafe {
                    *result = CResult {
                        success: false,
                        error_message: err_msg.into_raw(),
                        is_retryable: false,
                    };
                }
            }
            ptr::null_mut()
        }
    }
}

/// Free a stream instance
#[no_mangle]
pub extern "C" fn zerobus_stream_free(stream: *mut CZerobusStream) {
    if !stream.is_null() {
        unsafe {
            let _ = Box::from_raw(stream as *mut ZerobusStream);
        }
    }
}

/// Ingest a record (protobuf encoded)
/// Returns the offset ID on success, or -1 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_proto_record(
    stream: *mut CZerobusStream,
    data: *const u8,
    data_len: usize,
    result: *mut CResult,
) -> i64 {
    if stream.is_null() || data.is_null() {
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("Invalid pointer").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        return -1;
    }

    let stream_ref = unsafe { &*(stream as *const ZerobusStream) };
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };
    let data_vec = data_slice.to_vec();

    let res = RUNTIME.block_on(async {
        let payload = RecordPayload::Proto(data_vec);
        let ack_future = stream_ref.ingest_record(payload).await?;
        let offset = ack_future.await?;
        Ok::<i64, ZerobusError>(offset)
    });

    match res {
        Ok(offset) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            offset
        }
        Err(err) => {
            if !result.is_null() {
                unsafe { *result = CResult::error(err); }
            }
            -1
        }
    }
}

/// Ingest a JSON record
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_json_record(
    stream: *mut CZerobusStream,
    json_data: *const c_char,
    result: *mut CResult,
) -> i64 {
    if stream.is_null() || json_data.is_null() {
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("Invalid pointer").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        return -1;
    }

    let stream_ref = unsafe { &*(stream as *const ZerobusStream) };

    let res = (|| -> Result<i64, String> {
        let json_str = unsafe { c_str_to_string(json_data).map_err(|e| e.to_string())? };

        RUNTIME.block_on(async {
            let payload = RecordPayload::Json(json_str);
            let ack_future = stream_ref.ingest_record(payload).await.map_err(|e| e.to_string())?;
            let offset = ack_future.await.map_err(|e| e.to_string())?;
            Ok(offset)
        })
    })();

    match res {
        Ok(offset) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            offset
        }
        Err(err) => {
            if !result.is_null() {
                let err_msg = CString::new(err).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
                unsafe {
                    *result = CResult {
                        success: false,
                        error_message: err_msg.into_raw(),
                        is_retryable: false,
                    };
                }
            }
            -1
        }
    }
}

/// Flush all pending records
#[no_mangle]
pub extern "C" fn zerobus_stream_flush(
    stream: *mut CZerobusStream,
    result: *mut CResult,
) -> bool {
    if stream.is_null() {
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("Stream pointer is null").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        return false;
    }

    let stream_ref = unsafe { &*(stream as *const ZerobusStream) };

    let res = RUNTIME.block_on(async {
        stream_ref.flush().await
    });

    match res {
        Ok(_) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            true
        }
        Err(err) => {
            if !result.is_null() {
                unsafe { *result = CResult::error(err); }
            }
            false
        }
    }
}

/// Close the stream gracefully
#[no_mangle]
pub extern "C" fn zerobus_stream_close(
    stream: *mut CZerobusStream,
    result: *mut CResult,
) -> bool {
    if stream.is_null() {
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("Stream pointer is null").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        return false;
    }

    let stream_ref = unsafe { &mut *(stream as *mut ZerobusStream) };

    let res = RUNTIME.block_on(async {
        stream_ref.close().await
    });

    match res {
        Ok(_) => {
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            true
        }
        Err(err) => {
            if !result.is_null() {
                unsafe { *result = CResult::error(err); }
            }
            false
        }
    }
}

/// Free error message string
#[no_mangle]
pub extern "C" fn zerobus_free_error_message(message: *mut c_char) {
    if !message.is_null() {
        unsafe {
            let _ = CString::from_raw(message);
        }
    }
}

/// Get default configuration options
#[no_mangle]
pub extern "C" fn zerobus_get_default_config() -> CStreamConfigurationOptions {
    let default_opts = StreamConfigurationOptions::default();
    CStreamConfigurationOptions {
        max_inflight_records: default_opts.max_inflight_records,
        recovery: default_opts.recovery,
        recovery_timeout_ms: default_opts.recovery_timeout_ms,
        recovery_backoff_ms: default_opts.recovery_backoff_ms,
        recovery_retries: default_opts.recovery_retries,
        server_lack_of_ack_timeout_ms: default_opts.server_lack_of_ack_timeout_ms,
        flush_timeout_ms: default_opts.flush_timeout_ms,
        record_type: 1, // RecordType::Proto
    }
}
