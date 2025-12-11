# Zerobus Go SDK

A Go wrapper for the Databricks Zerobus streaming ingestion SDK. This SDK provides a high-performance interface for ingesting data into Databricks Delta tables using the Zerobus service.

## Overview

This Go SDK wraps the Rust [zerobus-sdk-rs](https://github.com/databricks/zerobus-sdk-rs) using CGO and FFI (Foreign Function Interface). It provides an idiomatic Go API while leveraging the performance and reliability of the underlying Rust implementation.

## Features

- **High-throughput streaming ingestion** into Databricks Delta tables
- **Automatic OAuth 2.0 authentication** with Unity Catalog
- **Simple JSON ingestion** - No code generation required!
- **Backpressure control** to manage memory usage
- **Automatic retry and recovery** for transient failures
- **Protocol Buffers support** for advanced use cases
- **Async acknowledgments** for ingested records
- **Configurable timeouts and retry policies**

## Architecture

```
┌─────────────────┐
│   Go SDK        │  ← Idiomatic Go API (this package)
│   (zerobus)     │
└────────┬────────┘
         │ CGO
┌────────▼────────┐
│  Rust FFI Crate │  ← C-compatible wrapper
│ (zerobus-ffi)   │
└────────┬────────┘
         │
┌────────▼────────┐
│  Rust SDK       │  ← Core implementation
│(zerobus-sdk-rs) │
└─────────────────┘
```

## Prerequisites

- **Go 1.19+**
- **CGO enabled** (required for calling Rust code)
- **Rust toolchain** (for building the FFI layer)
- The **libzerobus_ffi.so** shared library (built from `zerobus-ffi` crate)

## Installation

## Building from Source

```bash
# Clone and build
git clone <repository>
cd zerobus-go-sdk
./build.sh

# Set library path
source setup.sh  # Created by build.sh
```

## Installation

This SDK is distributed as platform-specific release packages (`.tar.gz` files).

**For end users:** See `INSTALL.md` in the release package for setup instructions.

**For developers:** Clone this repository and run `./build.sh`

## Quick Start

### JSON Ingestion (Recommended)

```go
package main

import (
    "log"
    zerobus "github.com/databricks/zerobus-go-sdk"
)

func main() {
    // Create SDK instance
    sdk, err := zerobus.NewZerobusSdk(
        "https://zerobus.databricks.com",
        "https://workspace.databricks.com",
    )
    if err != nil {
        log.Fatal(err)
    }
    defer sdk.Free()

    // Configure for JSON records
    options := zerobus.DefaultStreamConfigurationOptions()
    options.RecordType = zerobus.RecordTypeJson

    // Create stream
    stream, err := sdk.CreateStream(
        zerobus.TableProperties{
            TableName: "catalog.schema.table",
        },
        "your-client-id",
        "your-client-secret",
        options,
    )
    if err != nil {
        log.Fatal(err)
    }
    defer stream.Close()

    // Ingest records
    offset, err := stream.IngestRecord(`{"id": 1, "message": "Hello"}`)
    if err != nil {
        log.Fatal(err)
    }
    log.Printf("Ingested record at offset %d", offset)

    // Flush to ensure durability
    if err := stream.Flush(); err != nil {
        log.Fatal(err)
    }
}
```

### Protocol Buffer Ingestion

```go
import (
    "google.golang.org/protobuf/proto"
    "google.golang.org/protobuf/types/descriptorpb"
)

// Create protobuf descriptor
descriptor := &descriptorpb.DescriptorProto{
    Name: proto.String("MyMessage"),
    Field: []*descriptorpb.FieldDescriptorProto{
        {
            Name:   proto.String("id"),
            Number: proto.Int32(1),
            Type:   descriptorpb.FieldDescriptorProto_TYPE_INT64.Enum(),
        },
    },
}
descriptorBytes, _ := proto.Marshal(descriptor)

// Create stream for Proto records
options := zerobus.DefaultStreamConfigurationOptions()
options.UseJSONRecordType = false // Default

stream, err := sdk.CreateStream(
    zerobus.TableProperties{
        TableName:       "catalog.schema.table",
        DescriptorProto: descriptorBytes,
    },
    clientID,
    clientSecret,
    options,
)

// Ingest proto-encoded records
offset, err := stream.IngestProtoRecord(protoBytes)
```

## API Reference

### ZerobusSdk

The main SDK entry point for managing connections.

#### `NewZerobusSdk(zerobusEndpoint, unityCatalogURL string) (*ZerobusSdk, error)`

Creates a new SDK instance.

- `zerobusEndpoint`: Zerobus gRPC service endpoint
- `unityCatalogURL`: Unity Catalog URL for OAuth token acquisition

#### `CreateStream(tableProps, clientID, clientSecret string, options) (*ZerobusStream, error)`

Creates a new ingestion stream with OAuth authentication.

- `tableProps`: Table properties (name and optional descriptor)
- `clientID`, `clientSecret`: OAuth 2.0 credentials
- `options`: Stream configuration (nil for defaults)

#### `Free()`

Explicitly releases SDK resources. Called automatically by finalizer.

### ZerobusStream

Represents an active bidirectional streaming connection.

#### `IngestProtoRecord(data []byte) (int64, error)`

Ingests a Protocol Buffer encoded record.

Returns the logical offset ID assigned to the record.

#### `IngestJSONRecord(jsonData string) (int64, error)`

Ingests a JSON-encoded record.

Returns the logical offset ID assigned to the record.

#### `Flush() error`

Blocks until all pending records are acknowledged by the server.

#### `Close() error`

Gracefully closes the stream after flushing pending records.

### StreamConfigurationOptions

Configuration for stream behavior.

```go
type StreamConfigurationOptions struct {
    MaxInflightRecords       uint64 // Default: 1,000,000
    Recovery                 bool   // Default: true
    RecoveryTimeoutMs        uint64 // Default: 15000
    RecoveryBackoffMs        uint64 // Default: 2000
    RecoveryRetries          uint32 // Default: 4
    ServerLackOfAckTimeoutMs uint64 // Default: 60000
    FlushTimeoutMs           uint64 // Default: 300000
    UseJSONRecordType        bool   // Default: false
}
```

### Error Handling

```go
offset, err := stream.IngestJSONRecord(data)
if err != nil {
    if zerobusErr, ok := err.(*zerobus.ZerobusError); ok {
        if zerobusErr.Retryable() {
            // Retry logic for transient failures
        }
    }
}
```

## Configuration

### Environment Variables

```bash
export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
export DATABRICKS_CLIENT_ID="your-oauth-client-id"
export DATABRICKS_CLIENT_SECRET="your-oauth-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.table"
```

### Stream Options

Customize behavior with `StreamConfigurationOptions`:

```go
options := zerobus.DefaultStreamConfigurationOptions()
options.MaxInflightRecords = 50000  // Lower for memory-constrained environments
options.RecoveryRetries = 10         // More retries for unreliable networks
options.FlushTimeoutMs = 600000      // 10 minute flush timeout
```

## Examples

See the `examples/` directory for complete working examples:

- **`basic_json_usage.go`** - JSON ingestion (recommended, simple)
- **`basic_proto_usage.go`** - Protocol Buffer ingestion (advanced)

Run an example:

```bash
cd examples
go run basic_json_usage.go
```

## Performance Tips

1. **Batch ingestion**: Ingest multiple records concurrently for best throughput
2. **Adjust MaxInflightRecords**: Balance memory usage vs throughput
3. **Use Protocol Buffers**: More efficient than JSON for high-volume scenarios
4. **Explicit Close()**: Call `stream.Close()` explicitly rather than relying on finalizers

## Troubleshooting

### CGO Errors

```
could not determine kind of name for C.zerobus_sdk_new
```

**Solution**: Ensure `libzerobus_ffi.so` is in `LD_LIBRARY_PATH` or copied to a system library directory.

### Authentication Errors

```
ZerobusError: Unauthenticated
```

**Solution**: Verify OAuth client credentials and Unity Catalog URL. Ensure the client has permissions for the target table.

### Build Errors

```
undefined reference to `zerobus_sdk_new'
```

**Solution**: Rebuild the Rust FFI layer with `cargo build --release` in the `zerobus-ffi` directory.

## Publishing

When publishing this SDK, reference the Rust SDK via git:

In `zerobus-ffi/Cargo.toml`:

```toml
[dependencies]
databricks-zerobus-ingest-sdk = { git = "https://github.com/databricks/zerobus-sdk-rs", tag = "v0.1.1" }
```

## License

This SDK wrapper follows the same license as the underlying Rust SDK. See LICENSE file for details.

## Contributing

Contributions are welcome! Please ensure:

1. Rust FFI layer builds successfully
2. Go code follows standard formatting (`go fmt`)
3. Examples run without errors
4. Documentation is updated for API changes

## Support

For issues related to:
- **This Go wrapper**: Open an issue in this repository
- **Underlying Rust SDK**: See [zerobus-sdk-rs](https://github.com/databricks/zerobus-sdk-rs)
- **Databricks Zerobus service**: Contact Databricks support
