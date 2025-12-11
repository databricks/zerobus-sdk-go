package zerobus

// #cgo linux LDFLAGS: -L${SRCDIR}/.. -lzerobus_ffi -ldl -lpthread -lm -lresolv
// #cgo darwin LDFLAGS: -L${SRCDIR}/.. -lzerobus_ffi -framework CoreFoundation
// #cgo CFLAGS: -I${SRCDIR}/zerobus-ffi
// #include "zerobus.h"
// #include <stdlib.h>
import "C"
import (
	"unsafe"
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
		max_inflight_records:          C.size_t(opts.MaxInflightRecords),
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

// streamFree frees a stream instance
func streamFree(ptr unsafe.Pointer) {
	if ptr != nil {
		C.zerobus_stream_free((*C.CZerobusStream)(ptr))
	}
}

// streamIngestProtoRecord ingests a protobuf record
func streamIngestProtoRecord(streamPtr unsafe.Pointer, data []byte) (int64, error) {
	if len(data) == 0 {
		return -1, &ZerobusError{Message: "empty data", IsRetryable: false}
	}

	cData := (*C.uint8_t)(unsafe.Pointer(&data[0]))
	dataLen := C.size_t(len(data))

	var cres C.CResult
	offset := C.zerobus_stream_ingest_proto_record(
		(*C.CZerobusStream)(streamPtr),
		cData,
		dataLen,
		&cres,
	)

	if offset < 0 {
		return -1, ffiResult(cres)
	}

	return int64(offset), nil
}

// streamIngestJSONRecord ingests a JSON record
func streamIngestJSONRecord(streamPtr unsafe.Pointer, jsonData string) (int64, error) {
	cJSON := C.CString(jsonData)
	defer C.free(unsafe.Pointer(cJSON))

	var cres C.CResult
	offset := C.zerobus_stream_ingest_json_record(
		(*C.CZerobusStream)(streamPtr),
		cJSON,
		&cres,
	)

	if offset < 0 {
		return -1, ffiResult(cres)
	}

	return int64(offset), nil
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
