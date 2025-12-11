use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

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

// Global acknowledgment registry
static ACK_COUNTER: AtomicU64 = AtomicU64::new(1);
static ACK_REGISTRY: Lazy<Mutex<HashMap<u64, JoinHandle<Result<i64, ZerobusError>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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

/// Ingest a record (protobuf encoded) - NON-BLOCKING
/// Returns an acknowledgment ID that can be awaited later
/// Returns 0 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_proto_record(
    stream: *mut CZerobusStream,
    data: *const u8,
    data_len: usize,
    result: *mut CResult,
) -> u64 {
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
        return 0;
    }

    let stream_ref = unsafe { &*(stream as *const ZerobusStream) };
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };
    let data_vec = data_slice.to_vec();

    // Queue the record and get the acknowledgment future
    let ack_future_res = RUNTIME.block_on(async {
        let payload = RecordPayload::Proto(data_vec);
        stream_ref.ingest_record(payload).await
    });

    match ack_future_res {
        Ok(ack_future) => {
            // Spawn a task to await the acknowledgment
            let ack_id = ACK_COUNTER.fetch_add(1, Ordering::SeqCst);
            let handle = RUNTIME.spawn(async move {
                ack_future.await
            });

            // Store the handle
            ACK_REGISTRY.lock().unwrap().insert(ack_id, handle);

            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            ack_id
        }
        Err(err) => {
            if !result.is_null() {
                unsafe { *result = CResult::error(err); }
            }
            0
        }
    }
}

/// Ingest a JSON record - NON-BLOCKING
/// Returns an acknowledgment ID that can be awaited later
/// Returns 0 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_json_record(
    stream: *mut CZerobusStream,
    json_data: *const c_char,
    result: *mut CResult,
) -> u64 {
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
        return 0;
    }

    let stream_ref = unsafe { &*(stream as *const ZerobusStream) };
    let json_str = match unsafe { c_str_to_string(json_data) } {
        Ok(s) => s,
        Err(e) => {
            if !result.is_null() {
                unsafe {
                    *result = CResult {
                        success: false,
                        error_message: CString::new(e).unwrap().into_raw(),
                        is_retryable: false,
                    };
                }
            }
            return 0;
        }
    };

    // Queue the record and get the acknowledgment future
    let ack_future_res = RUNTIME.block_on(async {
        let payload = RecordPayload::Json(json_str);
        stream_ref.ingest_record(payload).await
    });

    match ack_future_res {
        Ok(ack_future) => {
            // Spawn a task to await the acknowledgment
            let ack_id = ACK_COUNTER.fetch_add(1, Ordering::SeqCst);
            let handle = RUNTIME.spawn(async move {
                ack_future.await
            });

            // Store the handle
            ACK_REGISTRY.lock().unwrap().insert(ack_id, handle);

            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            ack_id
        }
        Err(err) => {
            if !result.is_null() {
                unsafe { *result = CResult::error(err); }
            }
            0
        }
    }
}

/// Await an acknowledgment (BLOCKING)
/// Returns the offset on success, or -1 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_await_ack(
    ack_id: u64,
    result: *mut CResult,
) -> i64 {
    // Remove the handle from the registry
    let handle = {
        let mut registry = ACK_REGISTRY.lock().unwrap();
        registry.remove(&ack_id)
    };

    match handle {
        Some(h) => {
            // Wait for the acknowledgment
            let res = RUNTIME.block_on(h);

            match res {
                Ok(Ok(offset)) => {
                    if !result.is_null() {
                        unsafe { *result = CResult::success(); }
                    }
                    offset
                }
                Ok(Err(err)) => {
                    if !result.is_null() {
                        unsafe { *result = CResult::error(err); }
                    }
                    -1
                }
                Err(_) => {
                    if !result.is_null() {
                        unsafe {
                            *result = CResult {
                                success: false,
                                error_message: CString::new("Task panicked").unwrap().into_raw(),
                                is_retryable: false,
                            };
                        }
                    }
                    -1
                }
            }
        }
        None => {
            if !result.is_null() {
                unsafe {
                    *result = CResult {
                        success: false,
                        error_message: CString::new("Invalid ack ID").unwrap().into_raw(),
                        is_retryable: false,
                    };
                }
            }
            -1
        }
    }
}

/// Try to get an acknowledgment without blocking
/// Returns:
///   offset >= 0: Acknowledgment ready with offset
///   -1: Still pending (check is_ready)
///   -2: Error occurred (check result)
#[no_mangle]
pub extern "C" fn zerobus_stream_try_get_ack(
    ack_id: u64,
    is_ready: *mut bool,
    result: *mut CResult,
) -> i64 {
    let registry = ACK_REGISTRY.lock().unwrap();

    if let Some(handle) = registry.get(&ack_id) {
        if handle.is_finished() {
            drop(registry);
            // Remove and get the result
            let handle = ACK_REGISTRY.lock().unwrap().remove(&ack_id).unwrap();
            let res = RUNTIME.block_on(handle);

            if !is_ready.is_null() {
                unsafe { *is_ready = true; }
            }

            match res {
                Ok(Ok(offset)) => {
                    if !result.is_null() {
                        unsafe { *result = CResult::success(); }
                    }
                    offset
                }
                Ok(Err(err)) => {
                    if !result.is_null() {
                        unsafe { *result = CResult::error(err); }
                    }
                    -2
                }
                Err(_) => {
                    if !result.is_null() {
                        unsafe {
                            *result = CResult {
                                success: false,
                                error_message: CString::new("Task panicked").unwrap().into_raw(),
                                is_retryable: false,
                            };
                        }
                    }
                    -2
                }
            }
        } else {
            // Still pending
            if !is_ready.is_null() {
                unsafe { *is_ready = false; }
            }
            if !result.is_null() {
                unsafe { *result = CResult::success(); }
            }
            -1
        }
    } else {
        // Invalid ID
        if !is_ready.is_null() {
            unsafe { *is_ready = false; }
        }
        if !result.is_null() {
            unsafe {
                *result = CResult {
                    success: false,
                    error_message: CString::new("Invalid ack ID").unwrap().into_raw(),
                    is_retryable: false,
                };
            }
        }
        -2
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
