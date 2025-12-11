package zerobus

import (
	"runtime"
	"unsafe"
)

// ZerobusSdk is the main entry point for interacting with the Zerobus ingestion service.
// It manages the connection to the Zerobus endpoint and Unity Catalog.
type ZerobusSdk struct {
	ptr unsafe.Pointer
}

// ZerobusStream represents an active bidirectional gRPC stream for ingesting records.
// Records can be ingested concurrently and will be acknowledged asynchronously.
type ZerobusStream struct {
	ptr unsafe.Pointer
}

// NewZerobusSdk creates a new SDK instance.
//
// Parameters:
//   - zerobusEndpoint: The gRPC endpoint for the Zerobus service (e.g., "https://zerobus.databricks.com")
//   - unityCatalogURL: The Unity Catalog URL for OAuth token acquisition (e.g., "https://workspace.databricks.com")
//
// Returns an error if:
//   - Invalid endpoint URLs
//   - Unable to extract workspace ID from Unity Catalog URL
func NewZerobusSdk(zerobusEndpoint, unityCatalogURL string) (*ZerobusSdk, error) {
	ptr, err := sdkNew(zerobusEndpoint, unityCatalogURL)
	if err != nil {
		return nil, err
	}

	sdk := &ZerobusSdk{ptr: ptr}

	// Set up finalizer for automatic cleanup
	runtime.SetFinalizer(sdk, func(s *ZerobusSdk) {
		s.Free()
	})

	return sdk, nil
}

// Free explicitly releases resources associated with the SDK.
// The SDK cannot be used after calling Free().
// Note: This is automatically called by the garbage collector, but can be called explicitly for deterministic cleanup.
func (s *ZerobusSdk) Free() {
	if s.ptr != nil {
		sdkFree(s.ptr)
		s.ptr = nil
	}
}

// CreateStream creates a new bidirectional gRPC stream for ingesting records into a Databricks table.
// This method uses OAuth 2.0 client credentials flow for authentication.
//
// Parameters:
//   - tableProps: Table properties including name and optional protobuf descriptor
//   - clientID: OAuth 2.0 client ID
//   - clientSecret: OAuth 2.0 client secret
//   - options: Stream configuration options (nil for defaults)
//
// Returns an error if:
//   - Invalid table name format
//   - Authentication fails
//   - Insufficient permissions
//   - Network connectivity issues
//
// Example:
//
//	stream, err := sdk.CreateStream(
//	    TableProperties{
//	        TableName: "catalog.schema.table",
//	        DescriptorProto: descriptorBytes,
//	    },
//	    clientID,
//	    clientSecret,
//	    nil, // use default options
//	)
func (s *ZerobusSdk) CreateStream(
	tableProps TableProperties,
	clientID string,
	clientSecret string,
	options *StreamConfigurationOptions,
) (*ZerobusStream, error) {
	if s.ptr == nil {
		return nil, &ZerobusError{Message: "SDK has been freed", IsRetryable: false}
	}

	ptr, err := sdkCreateStream(
		s.ptr,
		tableProps.TableName,
		tableProps.DescriptorProto,
		clientID,
		clientSecret,
		options,
	)
	if err != nil {
		return nil, err
	}

	stream := &ZerobusStream{ptr: ptr}

	// Set up finalizer for automatic cleanup
	runtime.SetFinalizer(stream, func(st *ZerobusStream) {
		st.Close()
	})

	return stream, nil
}

// IngestRecord ingests a record into the stream.
// This method blocks if the maximum number of in-flight records is reached (backpressure).
//
// The payload parameter accepts either:
//   - []byte for Protocol Buffer encoded records
//   - string for JSON encoded records
//
// Returns:
//   - offset: The logical offset ID assigned to this record
//   - error: Any error that occurred during ingestion
//
// The record type is automatically detected based on the payload type.
// Records are acknowledged asynchronously by the server.
//
// Examples:
//
//	// JSON record
//	offset, err := stream.IngestRecord(`{"field": "value"}`)
//
//	// Proto record
//	offset, err := stream.IngestRecord(protoBytes)
func (st *ZerobusStream) IngestRecord(payload interface{}) (int64, error) {
	if st.ptr == nil {
		return -1, &ZerobusError{Message: "Stream has been closed", IsRetryable: false}
	}

	switch v := payload.(type) {
	case []byte:
		return streamIngestProtoRecord(st.ptr, v)
	case string:
		return streamIngestJSONRecord(st.ptr, v)
	default:
		return -1, &ZerobusError{
			Message:     "Invalid payload type: must be []byte or string",
			IsRetryable: false,
		}
	}
}

// Flush blocks until all pending records have been acknowledged by the server.
// This ensures durability guarantees before proceeding.
//
// Returns an error if:
//   - Flush timeout is exceeded
//   - Any record fails with a non-retryable error
//
// Example:
//
//	if err := stream.Flush(); err != nil {
//	    log.Printf("Flush failed: %v", err)
//	}
func (st *ZerobusStream) Flush() error {
	if st.ptr == nil {
		return &ZerobusError{Message: "Stream has been closed", IsRetryable: false}
	}

	return streamFlush(st.ptr)
}

// Close gracefully closes the stream after flushing all pending records.
// This method ensures all records are durably stored before closing the connection.
//
// The stream cannot be used after calling Close().
// Note: This is automatically called by the garbage collector, but should be called explicitly
// when done with the stream to ensure timely resource cleanup and proper error handling.
//
// Returns an error if:
//   - Flush fails
//   - Unable to close the gRPC connection
//
// Example:
//
//	defer stream.Close()
func (st *ZerobusStream) Close() error {
	if st.ptr == nil {
		return nil // Already closed
	}

	err := streamClose(st.ptr)
	streamFree(st.ptr)
	st.ptr = nil

	return err
}
