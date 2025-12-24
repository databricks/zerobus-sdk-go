// Package zerobus provides a high-performance Go client for streaming data ingestion
// into Databricks Delta tables using the Zerobus service.
//
// Zerobus is a high-throughput streaming service for direct data ingestion into
// Databricks Delta tables, optimized for real-time data pipelines and high-volume workloads.
//
// # Installation
//
// This package requires a one-time build step to compile the Rust FFI layer:
//
//	go get github.com/databricks/zerobus-sdk-go
//	go generate github.com/databricks/zerobus-sdk-go
//
// Prerequisites: Go 1.19+, Rust 1.70+, CGO enabled
//
// # Quick Start
//
// Create an SDK instance and stream:
//
//	sdk, err := zerobus.NewZerobusSdk(
//	    "https://your-shard.zerobus.databricks.com",
//	    "https://your-workspace.databricks.com",
//	)
//	if err != nil {
//	    log.Fatal(err)
//	}
//	defer sdk.Free()
//
//	options := zerobus.DefaultStreamConfigurationOptions()
//	options.RecordType = zerobus.RecordTypeJson
//
//	stream, err := sdk.CreateStream(
//	    zerobus.TableProperties{TableName: "catalog.schema.table"},
//	    clientID,
//	    clientSecret,
//	    options,
//	)
//	if err != nil {
//	    log.Fatal(err)
//	}
//	defer stream.Close()
//
// # Ingesting Data
//
// JSON records:
//
//	ack, err := stream.IngestRecord(`{"id": 1, "message": "Hello"}`)
//	if err != nil {
//	    log.Fatal(err)
//	}
//	offset, err := ack.Await()
//
// Protocol Buffer records:
//
//	protoBytes, _ := proto.Marshal(myMessage)
//	ack, err := stream.IngestRecord(protoBytes)
//	offset, err := ack.Await()
//
// # Authentication
//
// The SDK supports OAuth 2.0 authentication with Unity Catalog:
//
//	stream, err := sdk.CreateStream(
//	    tableProps,
//	    os.Getenv("DATABRICKS_CLIENT_ID"),
//	    os.Getenv("DATABRICKS_CLIENT_SECRET"),
//	    options,
//	)
//
// For custom authentication, implement the HeadersProvider interface:
//
//	type CustomAuth struct{}
//
//	func (a *CustomAuth) GetHeaders() (map[string]string, error) {
//	    return map[string]string{
//	        "authorization": "Bearer " + getToken(),
//	        "x-databricks-zerobus-table-name": "catalog.schema.table",
//	    }, nil
//	}
//
//	stream, err := sdk.CreateStreamWithHeadersProvider(tableProps, &CustomAuth{}, options)
//
// # Error Handling
//
// Errors are categorized as retryable or non-retryable:
//
//	ack, err := stream.IngestRecord(data)
//	if err != nil {
//	    if zbErr, ok := err.(*zerobus.ZerobusError); ok {
//	        if zbErr.Retryable() {
//	            // Transient error, SDK will auto-recover
//	        } else {
//	            // Fatal error, manual intervention needed
//	        }
//	    }
//	}
//
// # Performance
//
// For high throughput, batch ingestion is recommended:
//
//	acks := make([]*zerobus.RecordAck, 0, 10000)
//	for i := 0; i < 10000; i++ {
//	    ack, _ := stream.IngestRecord(data)
//	    acks = append(acks, ack)
//	}
//	// Wait for all acknowledgments
//	for _, ack := range acks {
//	    offset, _ := ack.Await()
//	}
//
// # Static Linking
//
// This SDK uses static linking of the Rust FFI layer, resulting in self-contained
// Go binaries with no runtime dependencies or library path configuration needed.
//
// For more information, visit: https://github.com/databricks/zerobus-sdk-go
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

// HeadersProvider is an interface for providing custom authentication headers.
// Implement this interface to provide custom authentication logic.
//
// Example:
//
//	type CustomHeadersProvider struct{}
//
//	func (c *CustomHeadersProvider) GetHeaders() (map[string]string, error) {
//	    return map[string]string{
//	        "authorization": "Bearer custom-token",
//	        "x-databricks-zerobus-table-name": "catalog.schema.table",
//	    }, nil
//	}
type HeadersProvider interface {
	// GetHeaders returns the headers to be used for authentication.
	// This method will be called by the SDK when authentication is needed.
	GetHeaders() (map[string]string, error)
}

// CreateStreamWithHeadersProvider creates a new bidirectional gRPC stream using a custom headers provider.
// This is useful for testing or when you need custom authentication logic.
//
// Parameters:
//   - tableProps: Table properties including name and optional protobuf descriptor
//   - headersProvider: Custom implementation of HeadersProvider interface
//   - options: Stream configuration options (nil for defaults)
//
// Returns an error if:
//   - Invalid table name format
//   - Headers provider returns an error
//   - Network connectivity issues
//
// Example:
//
//	provider := &CustomHeadersProvider{}
//	stream, err := sdk.CreateStreamWithHeadersProvider(
//	    TableProperties{TableName: "catalog.schema.table"},
//	    provider,
//	    nil, // use default options
//	)
func (s *ZerobusSdk) CreateStreamWithHeadersProvider(
	tableProps TableProperties,
	headersProvider HeadersProvider,
	options *StreamConfigurationOptions,
) (*ZerobusStream, error) {
	if s.ptr == nil {
		return nil, &ZerobusError{Message: "SDK has been freed", IsRetryable: false}
	}

	ptr, err := sdkCreateStreamWithHeadersProvider(
		s.ptr,
		tableProps.TableName,
		tableProps.DescriptorProto,
		headersProvider,
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
// This method blocks until the record is queued with an acknowledgment that can be awaited later.
//
// The payload parameter accepts either:
//   - []byte for Protocol Buffer encoded records
//   - string for JSON encoded records
//
// Returns:
//   - *RecordAck: An acknowledgment that can be awaited for the offset
//   - error: Any error that occurred during queueing
//
// The record type is automatically detected based on the payload type.
// Records are acknowledged asynchronously by the server.
//
// Examples:
//
//	// Fire off multiple records without waiting
//	ack1 := stream.IngestRecord(`{"field": "value1"}`)
//	ack2 := stream.IngestRecord(`{"field": "value2"}`)
//	ack3 := stream.IngestRecord(`{"field": "value3"}`)
//
//	// Wait for acknowledgments
//	offset1, err1 := ack1.Await()
//	offset2, err2 := ack2.Await()
//	offset3, err3 := ack3.Await()
func (st *ZerobusStream) IngestRecord(payload interface{}) (*RecordAck, error) {
	if st.ptr == nil {
		return nil, &ZerobusError{Message: "Stream has been closed", IsRetryable: false}
	}

	var ackID uint64
	var err error

	switch v := payload.(type) {
	case []byte:
		ackID, err = streamIngestProtoRecord(st.ptr, v)
	case string:
		ackID, err = streamIngestJSONRecord(st.ptr, v)
	default:
		return nil, &ZerobusError{
			Message:     "Invalid payload type: must be []byte or string",
			IsRetryable: false,
		}
	}

	if err != nil {
		return nil, err
	}

	return &RecordAck{
		ackID: ackID,
	}, nil
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
