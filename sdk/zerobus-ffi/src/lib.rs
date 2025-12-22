use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::collections::{HashMap, HashSet};
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use databricks_zerobus_ingest_sdk::{
    ZerobusSdk, ZerobusStream, ZerobusError,
    TableProperties, StreamConfigurationOptions, EncodedRecord,
    HeadersProvider, ZerobusResult,
};
use databricks_zerobus_ingest_sdk::databricks::zerobus::RecordType;
use prost::Message;
use async_trait::async_trait;
use std::sync::Arc;

// Test module
#[cfg(test)]
mod tests;

// Global Tokio runtime for handling async Rust calls
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

// Global acknowledgment registry
static ACK_COUNTER: AtomicU64 = AtomicU64::new(1);
static ACK_REGISTRY: Lazy<Mutex<HashMap<u64, JoinHandle<Result<i64, ZerobusError>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Global cache for header keys to prevent memory leaks
// Header keys are typically a small set of constant strings (e.g., "Authorization", "Content-Type")
// We intern them once to avoid leaking memory on every callback
static HEADER_KEY_CACHE: Lazy<Mutex<HashSet<&'static str>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Intern a header key string to prevent memory leaks
/// Only leaks memory for unique keys, not on every call
pub(crate) fn intern_header_key(key: String) -> &'static str {
    let mut cache = HEADER_KEY_CACHE.lock().unwrap();
    
    // Check if we already have this key
    if let Some(&existing) = cache.iter().find(|&&k| k == key.as_str()) {
        return existing;
    }
    
    // Only leak if it's a new key (typically happens once per unique header name)
    let static_key: &'static str = Box::leak(key.into_boxed_str());
    cache.insert(static_key);
    static_key
}

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
    pub max_inflight_requests: usize,
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
        opts.max_inflight_requests = c_opts.max_inflight_requests;
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

/// A single header key-value pair for C FFI
#[repr(C)]
pub struct CHeader {
    pub key: *mut c_char,
    pub value: *mut c_char,
}

/// A collection of headers returned from Go callback
#[repr(C)]
pub struct CHeaders {
    pub headers: *mut CHeader,
    pub count: usize,
    pub error_message: *mut c_char,
}

/// Function pointer type for the headers provider callback
/// The callback should return a CHeaders struct
/// The caller is responsible for freeing the returned CHeaders using zerobus_free_headers
pub type HeadersProviderCallback = extern "C" fn(user_data: *mut std::ffi::c_void) -> CHeaders;

/// Free headers returned from callback
#[no_mangle]
pub extern "C" fn zerobus_free_headers(headers: CHeaders) {
    if !headers.headers.is_null() {
        unsafe {
            let headers_slice = std::slice::from_raw_parts_mut(headers.headers, headers.count);
            for header in headers_slice {
                if !header.key.is_null() {
                    let _ = CString::from_raw(header.key);
                }
                if !header.value.is_null() {
                    let _ = CString::from_raw(header.value);
                }
            }
            let _ = Vec::from_raw_parts(headers.headers, headers.count, headers.count);
        }
    }
    if !headers.error_message.is_null() {
        unsafe {
            let _ = CString::from_raw(headers.error_message);
        }
    }
}

/// Rust struct that wraps a Go callback and implements HeadersProvider
pub(crate) struct CallbackHeadersProvider {
    callback: HeadersProviderCallback,
    user_data: *mut std::ffi::c_void,
    in_use: AtomicBool,  // Track concurrent access to detect thread-safety issues
}

impl CallbackHeadersProvider {
    pub(crate) fn new(callback: HeadersProviderCallback, user_data: *mut std::ffi::c_void) -> Self {
        Self {
            callback,
            user_data,
            in_use: AtomicBool::new(false),
        }
    }
}

// Safety: We assume the Go callback is thread-safe, but we validate at runtime
unsafe impl Send for CallbackHeadersProvider {}
unsafe impl Sync for CallbackHeadersProvider {}

#[async_trait]
impl HeadersProvider for CallbackHeadersProvider {
    async fn get_headers(&self) -> ZerobusResult<HashMap<&'static str, String>> {
        // Check for concurrent access (indicates thread-safety issue)
        if self.in_use.swap(true, Ordering::SeqCst) {
            return Err(ZerobusError::InvalidArgument(
                "Concurrent headers provider callback detected - Go callback must be thread-safe".to_string()
            ));
        }
        
        // Call the Go callback (synchronous)
        let c_headers = (self.callback)(self.user_data);
        
        // Release the lock before processing
        self.in_use.store(false, Ordering::SeqCst);

        // Check for error
        if !c_headers.error_message.is_null() {
            let error_str = unsafe {
                CStr::from_ptr(c_headers.error_message)
                    .to_string_lossy()
                    .into_owned()
            };
            zerobus_free_headers(c_headers);
            return Err(ZerobusError::InvalidArgument(format!("Headers provider error: {}", error_str)));
        }

        // Convert C headers to Rust HashMap
        let mut headers = HashMap::new();
        if !c_headers.headers.is_null() && c_headers.count > 0 {
            unsafe {
                let headers_slice = std::slice::from_raw_parts(c_headers.headers, c_headers.count);
                for header in headers_slice {
                    if !header.key.is_null() && !header.value.is_null() {
                        let key = CStr::from_ptr(header.key).to_string_lossy().into_owned();
                        let value = CStr::from_ptr(header.value).to_string_lossy().into_owned();

                        // Use interned keys to minimize memory leaks
                        // Only unique header names are leaked (typically < 10 strings for lifetime of process)
                        let static_key = intern_header_key(key);
                        headers.insert(static_key, value);
                    }
                }
            }
        }

        zerobus_free_headers(c_headers);
        Ok(headers)
    }
}

// ============================================================================
// SDK Functions
// ============================================================================

/// Safe wrapper to validate SDK pointer
pub(crate) fn validate_sdk_ptr<'a>(sdk: *mut CZerobusSdk) -> Result<&'a ZerobusSdk, &'static str> {
    if sdk.is_null() {
        return Err("SDK pointer is null");
    }
    // Still unsafe, but centralized and validated
    unsafe { Ok(&*(sdk as *const ZerobusSdk)) }
}

/// Safe wrapper to validate mutable SDK pointer
pub(crate) fn validate_sdk_ptr_mut<'a>(sdk: *mut CZerobusSdk) -> Result<&'a mut ZerobusSdk, &'static str> {
    if sdk.is_null() {
        return Err("SDK pointer is null");
    }
    unsafe { Ok(&mut *(sdk as *mut ZerobusSdk)) }
}

/// Safe wrapper to validate stream pointer
pub(crate) fn validate_stream_ptr<'a>(stream: *mut CZerobusStream) -> Result<&'a ZerobusStream, &'static str> {
    if stream.is_null() {
        return Err("Stream pointer is null");
    }
    unsafe { Ok(&*(stream as *const ZerobusStream)) }
}

/// Safe wrapper to validate mutable stream pointer
pub(crate) fn validate_stream_ptr_mut<'a>(stream: *mut CZerobusStream) -> Result<&'a mut ZerobusStream, &'static str> {
    if stream.is_null() {
        return Err("Stream pointer is null");
    }
    unsafe { Ok(&mut *(stream as *mut ZerobusStream)) }
}

/// Helper to write error result
pub(crate) fn write_error_result(result: *mut CResult, message: &str, is_retryable: bool) {
    if !result.is_null() {
        unsafe {
            *result = CResult {
                success: false,
                error_message: CString::new(message).unwrap_or_else(|_| CString::new("Error message contains null byte").unwrap()).into_raw(),
                is_retryable,
            };
        }
    }
}

/// Helper to write success result
pub(crate) fn write_success_result(result: *mut CResult) {
    if !result.is_null() {
        unsafe { *result = CResult::success(); }
    }
}

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

/// Set whether to use TLS for connections
/// This should be set to false when using HTTP endpoints (e.g., for testing)
#[no_mangle]
pub extern "C" fn zerobus_sdk_set_use_tls(sdk: *mut CZerobusSdk, use_tls: bool) {
    if let Ok(sdk_mut) = validate_sdk_ptr_mut(sdk) {
        sdk_mut.use_tls = use_tls;
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
    let sdk_ref = match validate_sdk_ptr(sdk) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return ptr::null_mut();
        }
    };

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
            write_success_result(result);
            stream_ptr
        }
        Err(err) => {
            write_error_result(result, &err, false);
            ptr::null_mut()
        }
    }
}

/// Create a stream with a custom headers provider callback
/// This allows you to provide custom authentication headers via a Go callback function
#[no_mangle]
pub extern "C" fn zerobus_sdk_create_stream_with_headers_provider(
    sdk: *mut CZerobusSdk,
    table_name: *const c_char,
    descriptor_proto_bytes: *const u8,
    descriptor_proto_len: usize,
    headers_callback: HeadersProviderCallback,
    user_data: *mut std::ffi::c_void,
    options: *const CStreamConfigurationOptions,
    result: *mut CResult,
) -> *mut CZerobusStream {
    let sdk_ref = match validate_sdk_ptr(sdk) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return ptr::null_mut();
        }
    };

    let res = RUNTIME.block_on(async {
        let table_name_str = unsafe { c_str_to_string(table_name).map_err(|e| e.to_string())? };

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

        // Create the headers provider from the callback with thread-safety validation
        let headers_provider = Arc::new(CallbackHeadersProvider::new(headers_callback, user_data));

        let stream = sdk_ref
            .create_stream_with_headers_provider(table_props, headers_provider, stream_options)
            .await
            .map_err(|e| {
                e.to_string()
            })?;

        let boxed = Box::new(stream);
        Ok::<*mut CZerobusStream, String>(Box::into_raw(boxed) as *mut CZerobusStream)
    });

    match res {
        Ok(stream_ptr) => {
            write_success_result(result);
            stream_ptr
        }
        Err(err) => {
            write_error_result(result, &err, false);
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
/// Returns an acknowledgment ID that can be awaited later
/// Returns 0 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_proto_record(
    stream: *mut CZerobusStream,
    data: *const u8,
    data_len: usize,
    result: *mut CResult,
) -> u64 {
    if data.is_null() {
        write_error_result(result, "Invalid data pointer", false);
        return 0;
    }

    let stream_ref = match validate_stream_ptr(stream) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return 0;
        }
    };

    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };
    let data_vec = data_slice.to_vec();

    // Queue the record and get the acknowledgment future
    let ack_future_res = RUNTIME.block_on(async {
        let payload = EncodedRecord::Proto(data_vec);
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

            write_success_result(result);
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

/// Ingest a JSON record
/// Returns an acknowledgment ID that can be awaited later
/// Returns 0 on error
#[no_mangle]
pub extern "C" fn zerobus_stream_ingest_json_record(
    stream: *mut CZerobusStream,
    json_data: *const c_char,
    result: *mut CResult,
) -> u64 {
    let stream_ref = match validate_stream_ptr(stream) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return 0;
        }
    };

    let json_str = match unsafe { c_str_to_string(json_data) } {
        Ok(s) => s,
        Err(e) => {
            write_error_result(result, e, false);
            return 0;
        }
    };

    // Queue the record and get the acknowledgment future
    let ack_future_res = RUNTIME.block_on(async {
        let payload = EncodedRecord::Json(json_str);
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

            write_success_result(result);
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
    let stream_ref = match validate_stream_ptr(stream) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return false;
        }
    };

    let res = RUNTIME.block_on(async {
        stream_ref.flush().await
    });

    match res {
        Ok(_) => {
            write_success_result(result);
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
    let stream_ref = match validate_stream_ptr_mut(stream) {
        Ok(s) => s,
        Err(msg) => {
            write_error_result(result, msg, false);
            return false;
        }
    };

    let res = RUNTIME.block_on(async {
        stream_ref.close().await
    });

    match res {
        Ok(_) => {
            write_success_result(result);
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
        max_inflight_requests: default_opts.max_inflight_requests,
        recovery: default_opts.recovery,
        recovery_timeout_ms: default_opts.recovery_timeout_ms,
        recovery_backoff_ms: default_opts.recovery_backoff_ms,
        recovery_retries: default_opts.recovery_retries,
        server_lack_of_ack_timeout_ms: default_opts.server_lack_of_ack_timeout_ms,
        flush_timeout_ms: default_opts.flush_timeout_ms,
        record_type: 1, // RecordType::Proto
    }
}
