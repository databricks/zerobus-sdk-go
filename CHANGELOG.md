# Version changelog

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
