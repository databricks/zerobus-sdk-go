package zerobus

/*
#cgo linux LDFLAGS: ${SRCDIR}/libzerobus_ffi.a -ldl -lpthread -lm -lresolv -lgcc_s
#cgo darwin LDFLAGS: ${SRCDIR}/libzerobus_ffi.a -framework CoreFoundation -framework Security -liconv
#cgo windows LDFLAGS: ${SRCDIR}/libzerobus_ffi.a -lws2_32 -luserenv -lbcrypt -lntdll
#cgo CFLAGS: -I${SRCDIR}/zerobus-ffi -Wno-implicit-function-declaration

#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

// Forward declare opaque types
typedef struct CZerobusSdk CZerobusSdk;
typedef struct CZerobusStream CZerobusStream;

// Define result type
typedef struct CResult {
    bool success;
    char *error_message;
    bool is_retryable;
} CResult;

// Define headers types for callback
typedef struct CHeader {
    char *key;
    char *value;
} CHeader;

typedef struct CHeaders {
    struct CHeader *headers;
    uintptr_t count;
    char *error_message;
} CHeaders;

typedef struct CHeaders (*HeadersProviderCallback)(void *user_data);

// Define stream configuration options
typedef struct CStreamConfigurationOptions {
    uintptr_t max_inflight_requests;
    bool recovery;
    uint64_t recovery_timeout_ms;
    uint64_t recovery_backoff_ms;
    uint32_t recovery_retries;
    uint64_t server_lack_of_ack_timeout_ms;
    uint64_t flush_timeout_ms;
    int32_t record_type;
} CStreamConfigurationOptions;

// Forward declare functions we need
extern CZerobusSdk* zerobus_sdk_new(const char* zerobus_endpoint,
                                     const char* unity_catalog_url,
                                     CResult* result);
extern void zerobus_sdk_free(CZerobusSdk* sdk);
extern void zerobus_sdk_set_use_tls(CZerobusSdk* sdk, bool use_tls);
extern CZerobusStream* zerobus_sdk_create_stream(CZerobusSdk* sdk,
                                                   const char* table_name,
                                                   const uint8_t* descriptor_proto_bytes,
                                                   uintptr_t descriptor_proto_len,
                                                   const char* client_id,
                                                   const char* client_secret,
                                                   const CStreamConfigurationOptions* options,
                                                   CResult* result);
extern CZerobusStream* zerobus_sdk_create_stream_with_headers_provider(
    CZerobusSdk* sdk,
    const char* table_name,
    const uint8_t* descriptor_proto_bytes,
    uintptr_t descriptor_proto_len,
    HeadersProviderCallback headers_callback,
    void* user_data,
    const CStreamConfigurationOptions* options,
    CResult* result);
extern void zerobus_stream_free(CZerobusStream* stream);
extern uint64_t zerobus_stream_ingest_proto_record(CZerobusStream* stream,
                                                     const uint8_t* data,
                                                     uintptr_t data_len,
                                                     CResult* result);
extern uint64_t zerobus_stream_ingest_json_record(CZerobusStream* stream,
                                                    const char* json_data,
                                                    CResult* result);
extern int64_t zerobus_stream_await_ack(uint64_t ack_id, CResult* result);
extern int64_t zerobus_stream_try_get_ack(uint64_t ack_id, bool* is_ready, CResult* result);
extern bool zerobus_stream_flush(CZerobusStream* stream, CResult* result);
extern bool zerobus_stream_close(CZerobusStream* stream, CResult* result);
extern void zerobus_free_error_message(char* error_message);
extern CStreamConfigurationOptions zerobus_get_default_config();

// Forward declaration of Go function
extern void goGetHeaders(void* userData, CHeader** headers, uintptr_t* count, char** error);

// C callback that matches the HeadersProviderCallback signature
static CHeaders cHeadersCallback(void* userData) {
    CHeader* headers = NULL;
    uintptr_t count = 0;
    char* error = NULL;

    // Call Go function
    goGetHeaders(userData, &headers, &count, &error);

    CHeaders result;
    result.headers = headers;
    result.count = count;
    result.error_message = error;
    return result;
}

// Helper function to get the C callback function pointer
static HeadersProviderCallback getHeadersCallback() {
    return (HeadersProviderCallback)cHeadersCallback;
}
*/
import "C"
import (
	"runtime/cgo"
	"sync"
	"unsafe"
)

// Registry to map stream pointers to their handles for cleanup
// This allows us to properly release cgo.Handle when streams are freed
var (
	streamHandleRegistry   = make(map[unsafe.Pointer]cgo.Handle)
	streamHandleRegistryMu sync.Mutex
)

// ffiResult converts a C.CResult to a Go error
func ffiResult(cres C.CResult) error {
	if cres.success {
		return nil
	}

	var message string
	if cres.error_message != nil {
		message = C.GoString(cres.error_message)
		C.zerobus_free_error_message(cres.error_message)
	} else {
		message = "unknown error"
	}

	return &ZerobusError{
		Message:     message,
		IsRetryable: bool(cres.is_retryable),
	}
}

// convertConfigToC converts Go config to C config
func convertConfigToC(opts *StreamConfigurationOptions) C.CStreamConfigurationOptions {
	if opts == nil {
		return C.zerobus_get_default_config()
	}

	return C.CStreamConfigurationOptions{
		max_inflight_requests:         C.size_t(opts.MaxInflightRequests),
		recovery:                      C.bool(opts.Recovery),
		recovery_timeout_ms:           C.uint64_t(opts.RecoveryTimeoutMs),
		recovery_backoff_ms:           C.uint64_t(opts.RecoveryBackoffMs),
		recovery_retries:              C.uint32_t(opts.RecoveryRetries),
		server_lack_of_ack_timeout_ms: C.uint64_t(opts.ServerLackOfAckTimeoutMs),
		flush_timeout_ms:              C.uint64_t(opts.FlushTimeoutMs),
		record_type:                   C.int(opts.RecordType),
	}
}

// sdkNew creates a new SDK instance via FFI
func sdkNew(zerobusEndpoint, unityCatalogURL string) (unsafe.Pointer, error) {
	cEndpoint := C.CString(zerobusEndpoint)
	defer C.free(unsafe.Pointer(cEndpoint))

	cCatalogURL := C.CString(unityCatalogURL)
	defer C.free(unsafe.Pointer(cCatalogURL))

	var cres C.CResult
	ptr := C.zerobus_sdk_new(cEndpoint, cCatalogURL, &cres)

	if ptr == nil {
		return nil, ffiResult(cres)
	}

	// Disable TLS if using HTTP endpoint (for testing/mock servers)
	if len(zerobusEndpoint) >= 7 && zerobusEndpoint[:7] == "http://" {
		C.zerobus_sdk_set_use_tls(ptr, C.bool(false))
	}

	return unsafe.Pointer(ptr), nil
}

// sdkFree frees an SDK instance
func sdkFree(ptr unsafe.Pointer) {
	if ptr != nil {
		C.zerobus_sdk_free((*C.CZerobusSdk)(ptr))
	}
}

// sdkCreateStream creates a stream via FFI
func sdkCreateStream(
	sdkPtr unsafe.Pointer,
	tableName string,
	descriptorProto []byte,
	clientID string,
	clientSecret string,
	options *StreamConfigurationOptions,
) (unsafe.Pointer, error) {
	cTableName := C.CString(tableName)
	defer C.free(unsafe.Pointer(cTableName))

	cClientID := C.CString(clientID)
	defer C.free(unsafe.Pointer(cClientID))

	cClientSecret := C.CString(clientSecret)
	defer C.free(unsafe.Pointer(cClientSecret))

	var cDescriptor *C.uint8_t
	var descriptorLen C.size_t

	if len(descriptorProto) > 0 {
		cDescriptor = (*C.uint8_t)(unsafe.Pointer(&descriptorProto[0]))
		descriptorLen = C.size_t(len(descriptorProto))
	}

	cOpts := convertConfigToC(options)

	var cres C.CResult
	ptr := C.zerobus_sdk_create_stream(
		(*C.CZerobusSdk)(sdkPtr),
		cTableName,
		cDescriptor,
		descriptorLen,
		cClientID,
		cClientSecret,
		&cOpts,
		&cres,
	)

	if ptr == nil {
		return nil, ffiResult(cres)
	}

	return unsafe.Pointer(ptr), nil
}

//export goGetHeaders
func goGetHeaders(userData unsafe.Pointer, headers **C.CHeader, count *C.uintptr_t, errorMsg **C.char) {
	// Convert userData back to cgo.Handle and retrieve the provider
	handle := cgo.Handle(userData)
	provider, ok := handle.Value().(HeadersProvider)

	if !ok {
		*errorMsg = C.CString("Invalid headers provider handle")
		*headers = nil
		*count = 0
		return
	}

	// Call the Go interface method
	headersMap, err := provider.GetHeaders()
	if err != nil {
		*errorMsg = C.CString(err.Error())
		*headers = nil
		*count = 0
		return
	}

	// Convert Go map to C array
	if len(headersMap) == 0 {
		*headers = nil
		*count = 0
		*errorMsg = nil
		return
	}

	// Allocate C array
	cHeaders := C.malloc(C.size_t(len(headersMap)) * C.size_t(unsafe.Sizeof(C.CHeader{})))
	cHeadersSlice := (*[1 << 30]C.CHeader)(cHeaders)[:len(headersMap):len(headersMap)]

	idx := 0
	for key, value := range headersMap {
		cHeadersSlice[idx].key = C.CString(key)
		cHeadersSlice[idx].value = C.CString(value)
		idx++
	}

	*headers = (*C.CHeader)(cHeaders)
	*count = C.uintptr_t(len(headersMap))
	*errorMsg = nil
}

// sdkCreateStreamWithHeadersProvider creates a stream with custom headers provider via FFI
func sdkCreateStreamWithHeadersProvider(
	sdkPtr unsafe.Pointer,
	tableName string,
	descriptorProto []byte,
	headersProvider HeadersProvider,
	options *StreamConfigurationOptions,
) (unsafe.Pointer, error) {
	cTableName := C.CString(tableName)
	defer C.free(unsafe.Pointer(cTableName))

	var cDescriptor *C.uint8_t
	var descriptorLen C.size_t

	if len(descriptorProto) > 0 {
		cDescriptor = (*C.uint8_t)(unsafe.Pointer(&descriptorProto[0]))
		descriptorLen = C.size_t(len(descriptorProto))
	}

	// Create a cgo.Handle for the provider
	// This keeps it alive and gives us a safe uintptr to pass to C
	handle := cgo.NewHandle(headersProvider)
	// Convert handle to unsafe.Pointer using a pattern the linter accepts
	handlePtr := *(*unsafe.Pointer)(unsafe.Pointer(&handle))

	cOpts := convertConfigToC(options)

	var cres C.CResult
	ptr := C.zerobus_sdk_create_stream_with_headers_provider(
		(*C.CZerobusSdk)(sdkPtr),
		cTableName,
		cDescriptor,
		descriptorLen,
		C.getHeadersCallback(),
		handlePtr,
		&cOpts,
		&cres,
	)

	if ptr == nil {
		// Clean up handle on error
		handle.Delete()
		return nil, ffiResult(cres)
	}

	// Store the handle so we can clean it up when the stream is freed
	streamHandleRegistryMu.Lock()
	streamHandleRegistry[unsafe.Pointer(ptr)] = handle
	streamHandleRegistryMu.Unlock()

	return unsafe.Pointer(ptr), nil
}

// streamFree frees a stream instance
func streamFree(ptr unsafe.Pointer) {
	if ptr != nil {
		// Clean up the associated handle BEFORE freeing the stream
		streamHandleRegistryMu.Lock()
		if handle, exists := streamHandleRegistry[ptr]; exists {
			handle.Delete() // This releases the Go object reference
			delete(streamHandleRegistry, ptr)
		}
		streamHandleRegistryMu.Unlock()

		C.zerobus_stream_free((*C.CZerobusStream)(ptr))
	}
}

// streamIngestProtoRecord ingests a protobuf record
// Returns an acknowledgment ID
func streamIngestProtoRecord(streamPtr unsafe.Pointer, data []byte) (uint64, error) {
	if len(data) == 0 {
		return 0, &ZerobusError{Message: "empty data", IsRetryable: false}
	}

	cData := (*C.uint8_t)(unsafe.Pointer(&data[0]))
	dataLen := C.size_t(len(data))

	var cres C.CResult
	ackID := C.zerobus_stream_ingest_proto_record(
		(*C.CZerobusStream)(streamPtr),
		cData,
		dataLen,
		&cres,
	)

	if ackID == 0 {
		return 0, ffiResult(cres)
	}

	return uint64(ackID), nil
}

// streamIngestJSONRecord ingests a JSON record
// Returns an acknowledgment ID
func streamIngestJSONRecord(streamPtr unsafe.Pointer, jsonData string) (uint64, error) {
	cJSON := C.CString(jsonData)
	defer C.free(unsafe.Pointer(cJSON))

	var cres C.CResult
	ackID := C.zerobus_stream_ingest_json_record(
		(*C.CZerobusStream)(streamPtr),
		cJSON,
		&cres,
	)

	if ackID == 0 {
		return 0, ffiResult(cres)
	}

	return uint64(ackID), nil
}

// streamAwaitAck waits for an acknowledgment and returns the offset
func streamAwaitAck(ackID uint64) (int64, error) {
	var cres C.CResult
	offset := C.zerobus_stream_await_ack(
		C.uint64_t(ackID),
		&cres,
	)

	if offset < 0 {
		return -1, ffiResult(cres)
	}

	return int64(offset), nil
}

// streamTryGetAck tries to get an acknowledgment without blocking
func streamTryGetAck(ackID uint64) (int64, error, bool) {
	var cres C.CResult
	var isReady C.bool

	offset := C.zerobus_stream_try_get_ack(
		C.uint64_t(ackID),
		&isReady,
		&cres,
	)

	if offset == -1 {
		// Still pending
		return 0, nil, false
	}

	if offset == -2 {
		// Error occurred
		return 0, ffiResult(cres), true
	}

	// Success
	return int64(offset), nil, true
}

// streamFlush flushes pending records
func streamFlush(streamPtr unsafe.Pointer) error {
	var cres C.CResult
	success := C.zerobus_stream_flush((*C.CZerobusStream)(streamPtr), &cres)

	if !success {
		return ffiResult(cres)
	}

	return nil
}

// streamClose closes the stream
func streamClose(streamPtr unsafe.Pointer) error {
	var cres C.CResult
	success := C.zerobus_stream_close((*C.CZerobusStream)(streamPtr), &cres)

	if !success {
		return ffiResult(cres)
	}

	return nil
}
