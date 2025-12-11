# Version changelog

## Release v0.2.0 (Unreleased)

### BREAKING CHANGES

- **Non-blocking IngestRecord API**: `IngestRecord()` now returns `(*RecordAck, error)` instead of `(int64, error)`
  - This eliminates the performance bottleneck where the SDK blocked on every single record until server acknowledgment
  - Use `ack.Await()` to get the offset when needed
  - Use `ack.TryGet()` for non-blocking status checks

**Migration Example:**

Before (v0.1.0):
```go
offset, err := stream.IngestRecord(data)
if err != nil {
    log.Fatal(err)
}
log.Printf("Offset: %d", offset)
```

After (v0.2.0):
```go
ack, err := stream.IngestRecord(data)
if err != nil {
    log.Fatal(err)
}

// Await acknowledgment
offset, err := ack.Await()
if err != nil {
    log.Fatal(err)
}
log.Printf("Offset: %d", offset)
```

### Features

- Added `RecordAck` type with `Await()` and `TryGet()` methods for flexible acknowledgment handling
- Global acknowledgment registry in Rust FFI layer for tracking async operations
- Non-blocking ingestion enables much higher throughput for batch operations

### Performance Improvements

- Eliminated blocking on every record acknowledgment, allowing concurrent ingestion
- Records can now be fired off rapidly without waiting for server responses
- Acknowledgments can be awaited selectively or batched for optimal performance

## Release v0.1.0

Initial release of the Databricks Zerobus Ingest SDK for Go.

### Features

- **Static Linking** - Self-contained binaries with no runtime dependencies
- **Go SDK wrapper** around the high-performance Rust implementation
- **CGO/FFI integration** for seamless Go-to-Rust interoperability
- **JSON ingestion** support for simple data streaming
- **Protocol Buffer ingestion** for type-safe, efficient data encoding
- **OAuth 2.0 authentication** with Unity Catalog integration
- **Automatic retry and recovery** for transient failures
- **Configurable stream options** including inflight limits, timeouts, and recovery behavior
- **Async acknowledgments** for tracking record ingestion

### API

- Added `ZerobusSdk` struct for creating and managing ingestion streams
- Added `ZerobusStream` for bidirectional gRPC streaming
- `IngestRecord()` method that accepts both JSON (string) and Protocol Buffer ([]byte) data
- Added `StreamConfigurationOptions` for fine-tuning stream behavior
- Added `ZerobusError` for detailed error handling with retryability detection
- `Flush()` method to ensure all pending records are acknowledged
- `Close()` method for graceful stream shutdown

### Build System

- Static library compilation for portability
- Platform detection for Linux and macOS
- Automated build scripts for development and release
- No LD_LIBRARY_PATH configuration required

### Documentation

- Comprehensive README with quick start examples
- JSON and Protocol Buffer usage examples
- API reference documentation
- Troubleshooting guide
- Performance optimization tips
