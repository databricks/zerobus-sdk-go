# Zerobus Go SDK

A high-performance Go client for streaming data ingestion into Databricks Delta tables using the Zerobus service.

## Disclaimer

[Public Preview](https://docs.databricks.com/release-notes/release-types.html): This SDK is supported for production use cases and is available to all customers. Databricks is actively working on stabilizing the Zerobus Ingest SDK for Go. Minor version updates may include backwards-incompatible changes.

We are keen to hear feedback from you on this SDK. Please [file issues](https://github.com/databricks/zerobus-go-sdk/issues), and we will address them.

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Repository Structure](#repository-structure)
- [Usage Guide](#usage-guide)
  - [1. Initialize the SDK](#1-initialize-the-sdk)
  - [2. Configure Authentication](#2-configure-authentication)
  - [3. Create a Stream](#3-create-a-stream)
  - [4. Ingest Data](#4-ingest-data)
  - [5. Handle Acknowledgments](#5-handle-acknowledgments)
  - [6. Close the Stream](#6-close-the-stream)
- [Configuration Options](#configuration-options)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [Best Practices](#best-practices)
- [API Reference](#api-reference)
- [Building from Source](#building-from-source)
- [Community and Contributing](#community-and-contributing)
- [License](#license)

## Overview

The Zerobus Go SDK provides a robust, CGO-based wrapper around the high-performance Rust implementation for ingesting large volumes of data into Databricks Delta tables. It abstracts the complexity of the Zerobus service and handles authentication, retries, stream recovery, and acknowledgment tracking automatically.

**What is Zerobus?** Zerobus is a high-throughput streaming service for direct data ingestion into Databricks Delta tables, optimized for real-time data pipelines and high-volume workloads.

This SDK wraps the Rust [zerobus-sdk-rs](https://github.com/databricks/zerobus-sdk-rs) using CGO and FFI (Foreign Function Interface), providing an idiomatic Go API while leveraging the performance and reliability of the underlying Rust implementation.

## Features

- **Static Linking** - Self-contained binaries with no runtime dependencies or LD_LIBRARY_PATH configuration
- **High-throughput streaming ingestion** into Databricks Delta tables
- **Automatic OAuth 2.0 authentication** with Unity Catalog
- **Simple JSON ingestion** - No code generation required for basic use cases
- **Protocol Buffers support** for type-safe, efficient data encoding
- **Backpressure control** to manage memory usage
- **Automatic retry and recovery** for transient failures
- **Configurable timeouts and retry policies**
- **Async acknowledgments** for ingested records
- **Graceful stream management** - Proper flushing and acknowledgment tracking

## Architecture

```
┌─────────────────┐
│   Go SDK        │  ← Idiomatic Go API (this package)
│   (zerobus)     │
└────────┬────────┘
         │ CGO (Static Linking)
┌────────▼────────┐
│  Rust FFI Crate │  ← C-compatible wrapper
│ (zerobus-ffi)   │
└────────┬────────┘
         │
┌────────▼────────┐
│  Rust SDK       │  ← Core async implementation (Tokio)
│(zerobus-sdk-rs) │     - gRPC bidirectional streaming
└────────┬────────┘     - OAuth 2.0 authentication
         │              - Automatic recovery
         ▼
   Databricks
 Zerobus Service
```

## Installation

### Prerequisites

- **Go 1.19+**
- **CGO enabled** (required for calling Rust code)
- **Rust toolchain** (for building from source)

### Quick Start

```bash
# 1. Get the SDK
go get github.com/databricks/zerobus-go-sdk

# 2. Build Rust FFI library (one-time, takes 2-5 minutes)
go generate github.com/databricks/zerobus-go-sdk/sdk

# 3. Use in your project!
```

That's it! After `go generate`, regular `go build` works normally.

### Adding to Your Project

In your `go.mod`:

```go
require github.com/databricks/zerobus-go-sdk v0.1.0
```

In your code:

```go
import zerobus "github.com/databricks/zerobus-go-sdk/sdk"

func main() {
    sdk, err := zerobus.NewZerobusSdk(endpoint, catalogURL)
    // ...
}
```

**First-time build:**

```bash
# In your project directory
go generate github.com/databricks/zerobus-go-sdk/sdk
go build
```

### For Local Development

```bash
# Clone and build
git clone https://github.com/databricks/zerobus-go-sdk.git
cd zerobus-go-sdk/sdk
go generate  # Builds Rust FFI
cd ..
make build   # Builds everything
```

## Quick Start

### JSON Ingestion (Recommended for Getting Started)

```go
package main

import (
    "log"
    zerobus "github.com/databricks/zerobus-go-sdk"
)

func main() {
    // Create SDK instance
    sdk, err := zerobus.NewZerobusSdk(
        "https://your-shard-id.zerobus.region.cloud.databricks.com",
        "https://your-workspace.cloud.databricks.com",
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

    // Ingest record (blocks until queued, returns ack handle)
    ack, err := stream.IngestRecord(`{"id": 1, "message": "Hello"}`)
    if err != nil {
        log.Fatal(err)
    }

    // Await acknowledgment to get offset
    offset, err := ack.Await()
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

### Protocol Buffer Ingestion (Recommended for Production)

```go
import (
    "google.golang.org/protobuf/proto"
    "google.golang.org/protobuf/types/descriptorpb"
)

// Load descriptor from generated files
descriptorBytes, err := os.ReadFile("path/to/schema.descriptor")
if err != nil {
    log.Fatal(err)
}

descriptor := &descriptorpb.DescriptorProto{}
if err := proto.Unmarshal(descriptorBytes, descriptor); err != nil {
    log.Fatal(err)
}

// Create stream for Proto records
options := zerobus.DefaultStreamConfigurationOptions()
options.RecordType = zerobus.RecordTypeProto

stream, err := sdk.CreateStream(
    zerobus.TableProperties{
        TableName:       "catalog.schema.table",
        DescriptorProto: descriptorBytes,
    },
    clientID,
    clientSecret,
    options,
)

// Ingest proto-encoded record (blocks until queued)
ack, err := stream.IngestRecord(protoBytes)

// Await acknowledgment to get offset
offset, err := ack.Await()
```

## Repository Structure

```
zerobus-go-sdk/
├── sdk/                            # Core SDK library
│   ├── zerobus.go                  # Main SDK and stream implementation
│   ├── ffi.go                      # CGO bindings to Rust FFI
│   ├── errors.go                   # Error types
│   ├── go.mod                      # Go module definition
│   └── zerobus-ffi/                # Rust FFI crate
│       ├── src/lib.rs              # FFI wrapper implementation
│       ├── zerobus.h               # C header for CGO
│       ├── Cargo.toml              # Rust dependencies
│       └── build.rs                # Build script for cbindgen
│
├── examples/                       # Working examples
│   ├── basic_example_json/                       # JSON ingestion example
│   │   ├── basic_json_usage.go     # JSON-based example
│   │   └── go.mod                  # Module file
│   └── basic_example_proto/                      # Protocol Buffer example
│       ├── basic_proto_usage.go    # Proto-based example
│       ├── air_quality.proto       # Example proto schema
│       ├── pb/                     # Generated proto code
│       └── go.mod                  # Module file
│
├── tests/                          # Test suite
│   └── README.md                   # Testing documentation
│
├── build.sh                        # Development build script
├── build_release.sh                # Release packaging script
├── Makefile                        # Build automation
├── README.md                       # This file
├── CHANGELOG.md                    # Version history
├── CONTRIBUTING.md                 # Contribution guidelines
├── SECURITY.md                     # Security policy
├── DCO                             # Developer Certificate of Origin
├── NOTICE                          # Third-party attribution
└── LICENSE                         # License file
```

### Key Components

- **`sdk/`** - The main library containing Go SDK and Rust FFI wrapper
- **`examples/`** - Complete working examples demonstrating SDK usage
- **`tests/`** - Integration and unit tests
- **`build.sh`** - Automated build script for development
- **`Makefile`** - Standard make targets for building, testing, and linting

## Usage Guide

### 1. Initialize the SDK

Create an SDK instance with your Databricks workspace endpoints:

```go
// For AWS
sdk, err := zerobus.NewZerobusSdk(
    "https://your-shard-id.zerobus.us-east-1.cloud.databricks.com",
    "https://your-workspace.cloud.databricks.com",
)

// For Azure
sdk, err := zerobus.NewZerobusSdk(
    "https://your-shard-id.zerobus.eastus.azuredatabricks.net",
    "https://your-workspace.azuredatabricks.net",
)

if err != nil {
    log.Fatal(err)
}
defer sdk.Free()
```

### 2. Configure Authentication

The SDK handles authentication automatically. You just need to provide your OAuth credentials:

```go
clientID := os.Getenv("DATABRICKS_CLIENT_ID")
clientSecret := os.Getenv("DATABRICKS_CLIENT_SECRET")
```

See the examples directory for how to obtain OAuth credentials.

### Custom Authentication

For advanced use cases, you can implement the `HeadersProvider` interface to supply your own authentication headers. This is useful for integrating with a different OAuth provider, using a centralized token caching service, or implementing alternative authentication mechanisms.

> **Note:** The headers you provide must still conform to the authentication protocol expected by the Zerobus service. The default OAuth implementation serves as the reference for the required headers (`authorization` and `x-databricks-zerobus-table-name`). This feature provides flexibility in *how* you source your credentials, not in changing the authentication protocol itself.

**Example:**

```go
import zerobus "github.com/databricks/zerobus-go-sdk"

// Implement the HeadersProvider interface.
type MyCustomAuthProvider struct {
    tableName string
}

func (p *MyCustomAuthProvider) GetHeaders() (map[string]string, error) {
    // Custom logic to fetch and cache a token would go here.
    return map[string]string{
        "authorization":                    "Bearer <your-token>",
        "x-databricks-zerobus-table-name": p.tableName,
    }, nil
}

func example(sdk *zerobus.ZerobusSdk, tableProps zerobus.TableProperties) error {
    customProvider := &MyCustomAuthProvider{tableName: "catalog.schema.table"}

    stream, err := sdk.CreateStreamWithHeadersProvider(
        tableProps,
        customProvider,
        nil,
    )
    if err != nil {
        return err
    }
    defer stream.Close()

    ack, _ := stream.IngestRecord(`{"data": "value"}`)
    offset, _ := ack.Await()
    return nil
}
```

**Common use cases:**

- **Token caching**: Implement custom token refresh logic
- **Alternative auth mechanisms**: Use different authentication providers
- **Dynamic credentials**: Fetch credentials on-demand from secret managers

### 3. Create a Stream

Configure table properties and stream options:

```go
options := zerobus.DefaultStreamConfigurationOptions()
options.MaxInflightRequests = 10000
options.Recovery = true
options.RecoveryRetries = 4
options.RecordType = zerobus.RecordTypeJson

stream, err := sdk.CreateStream(
    zerobus.TableProperties{
        TableName: "catalog.schema.table",
    },
    clientID,
    clientSecret,
    options,
)
if err != nil {
    log.Fatal(err)
}
defer stream.Close()
```

### 4. Ingest Data

**Single record:**

```go
// JSON (string) - blocks until queued, handles backpressure
ack, err := stream.IngestRecord(`{"id": 1, "value": "hello"}`)
if err != nil {
    log.Fatal(err)
}

// Await acknowledgment
offset, err := ack.Await()
if err != nil {
    log.Fatal(err)
}
log.Printf("Record ingested at offset: %d", offset)
```

**Batch ingestion for high throughput:**

```go
// Queue all records
acks := make([]*zerobus.RecordAck, 0, 100000)
for i := 0; i < 100000; i++ {
    jsonData := fmt.Sprintf(`{"id": %d, "timestamp": %d}`, i, time.Now().Unix())
    ack, err := stream.IngestRecord(jsonData)
    if err != nil {
        log.Fatal(err)
    }
    acks = append(acks, ack)
}

// Wait for server acknowledgments
for i, ack := range acks {
    if _, err := ack.Await(); err != nil {
        log.Printf("Record %d failed: %v", i, err)
    }
}
```

**For non-blocking concurrent ingestion, use goroutines:**

```go
var wg sync.WaitGroup
errCh := make(chan error, 100)

for i := 0; i < 100; i++ {
    wg.Add(1)
    go func(id int) {
        defer wg.Done()

        data := fmt.Sprintf(`{"id": %d}`, id)
        ack, err := stream.IngestRecord(data)  // Blocks this goroutine
        if err != nil {
            errCh <- err
            return
        }

        offset, err := ack.Await()
        if err != nil {
            errCh <- err
            return
        }
        log.Printf("Record %d acknowledged at offset %d", id, offset)
    }(id)
}

wg.Wait()
close(errCh)

// Check for errors
for err := range errCh {
    log.Printf("Ingestion error: %v", err)
}
```

**Concurrent ingestion with multiple streams:**

```go
var wg sync.WaitGroup
for partition := 0; partition < 4; partition++ {
    wg.Add(1)
    go func(p int) {
        defer wg.Done()

        // Each goroutine gets its own stream
        stream, err := sdk.CreateStream(tableProps, clientID, clientSecret, options)
        if err != nil {
            log.Fatal(err)
        }
        defer stream.Close()

        for i := p * 25000; i < (p+1)*25000; i++ {
            data := fmt.Sprintf(`{"id": %d}`, i)
            // Blocks until queued in this goroutine
            if _, err := stream.IngestRecord(data); err != nil {
                log.Fatal(err)
            }
            // Note: stream.Close() will flush and await all pending acks
        }
    }(p)
}
wg.Wait()
```

### 5. Handle Acknowledgments

After `IngestRecord()` returns, the record is queued. Use the returned acknowledgment to wait for server confirmation:

```go
offset, err := stream.IngestRecord(data)
if err != nil {
    // Handle ingestion error
    if zerobusErr, ok := err.(*zerobus.ZerobusError); ok {
        if zerobusErr.Retryable() {
            // Retry logic for transient failures
        }
    }
    log.Fatal(err)
}

// Offset is available immediately
log.Printf("Record committed at offset: %d", offset)
```

### 6. Close the Stream

Always close streams to ensure data is flushed:

```go
// Close gracefully (flushes automatically)
if err := stream.Close(); err != nil {
    log.Fatal(err)
}
```

## Configuration Options

### StreamConfigurationOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `MaxInflightRequests` | `uint64` | 1,000,000 | Maximum number of in-flight requests |
| `Recovery` | `bool` | true | Enable automatic stream recovery on failure |
| `RecoveryTimeoutMs` | `uint64` | 15,000 | Timeout for recovery operations (ms) |
| `RecoveryBackoffMs` | `uint64` | 2,000 | Delay between recovery retry attempts (ms) |
| `RecoveryRetries` | `uint32` | 4 | Maximum number of recovery attempts |
| `FlushTimeoutMs` | `uint64` | 300,000 | Timeout for flush operations (ms) |
| `ServerLackOfAckTimeoutMs` | `uint64` | 60,000 | Timeout waiting for server acks (ms) |
| `RecordType` | `int` | Proto | Record type: `RecordTypeProto` or `RecordTypeJson` |

**Example:**

```go
options := zerobus.DefaultStreamConfigurationOptions()
options.MaxInflightRequests = 50000
options.RecoveryRetries = 10
options.FlushTimeoutMs = 600000
options.RecordType = zerobus.RecordTypeJson
```

## Error Handling

The SDK categorizes errors as **retryable** or **non-retryable**:

### Retryable Errors
Auto-recovered if `Recovery` is enabled:
- Network failures
- Connection timeouts
- Temporary server errors
- Stream closed by server

### Non-Retryable Errors
Require manual intervention:
- Invalid OAuth credentials
- Invalid table name
- Schema mismatch
- Authentication failure
- Permission denied

**Check if an error is retryable:**

```go
offset, err := stream.IngestRecord(data)
if err != nil {
    if zerobusErr, ok := err.(*zerobus.ZerobusError); ok {
        if zerobusErr.Retryable() {
            log.Printf("Retryable error, SDK will auto-recover: %v", err)
            // Optionally implement custom retry logic
        } else {
            log.Fatalf("Fatal error, manual intervention needed: %v", err)
        }
    }
}
```

## Examples

The repository provides two complete examples in separate directories:

- **`examples/basic_example_json/`** - Simple JSON-based ingestion (recommended for getting started)
- **`examples/basic_example_proto/`** - Type-safe Protocol Buffer ingestion (recommended for production)

Run an example:

```bash
cd examples/basic_example_json
export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.table"
go run basic_json_usage.go
```

## Best Practices

1. **Reuse SDK Instances** - Create one `ZerobusSdk` per application and reuse for multiple streams
2. **Always Close Streams** - Use `defer stream.Close()` to ensure all data is flushed
3. **Tune Inflight Limits** - Adjust `MaxInflightRequests` based on memory and throughput needs
4. **Enable Recovery** - Always set `Recovery: true` in production environments
5. **Use Batch Ingestion** - For high throughput, ingest many records before calling `Flush()`
6. **Monitor Errors** - Log and alert on non-retryable errors
7. **Use Protocol Buffers for Production** - More efficient than JSON for high-volume scenarios
8. **Secure Credentials** - Never hardcode secrets; use environment variables or secret managers
9. **Test Recovery** - Simulate failures to verify your error handling logic
10. **One Stream Per Goroutine** - Don't share streams across goroutines; create separate streams for concurrent ingestion

## API Reference

### `ZerobusSdk`

Main entry point for the SDK.

#### `NewZerobusSdk(zerobusEndpoint, unityCatalogURL string) (*ZerobusSdk, error)`

Creates a new SDK instance.

- `zerobusEndpoint`: Zerobus gRPC service endpoint
- `unityCatalogURL`: Unity Catalog URL for OAuth token acquisition

#### `CreateStream(tableProps TableProperties, clientID, clientSecret string, options *StreamConfigurationOptions) (*ZerobusStream, error)`

Creates a new ingestion stream with OAuth authentication.

#### `CreateStreamWithHeadersProvider(tableProps TableProperties, headersProvider HeadersProvider, options *StreamConfigurationOptions) (*ZerobusStream, error)`

Creates a new ingestion stream with a custom headers provider for advanced authentication. Use this when you need custom authentication logic (e.g., custom token caching, or alternative auth providers).

**Example:**
```go
provider := &MyCustomAuthProvider{}
stream, err := sdk.CreateStreamWithHeadersProvider(
    tableProps,
    provider,
    options,
)
```

#### `Free()`

Explicitly releases SDK resources. Called automatically by finalizer.

### `ZerobusStream`

Represents an active bidirectional streaming connection.

#### `IngestRecord(payload interface{}) (*RecordAck, error)`

Ingests a record into the stream. **Blocks until the record is queued** (handles backpressure), then returns an acknowledgment handle for awaiting server confirmation. 

Accepts either:
- `string` for JSON-encoded records
- `[]byte` for Protocol Buffer-encoded records

Returns a `*RecordAck` that can be awaited to get the logical offset assigned to the record.

**Example:**
```go
ack, err := stream.IngestRecord(`{"id": 1}`)
if err != nil {
    // Handle queueing errors
}

// Wait for server acknowledgment
offset, err := ack.Await()
if err != nil {
    // Handle acknowledgment errors
}
```

**For non-blocking ingestion:**
```go
go func() {
    ack, _ := stream.IngestRecord(data)  // This goroutine blocks
    offset, _ := ack.Await()
    // Handle offset
}()
```

#### `Flush() error`

Blocks until all pending records are acknowledged by the server.

#### `Close() error`

Gracefully closes the stream after flushing pending records.

### `RecordAck`

Represents a pending acknowledgment for an ingested record.

#### `Await() (int64, error)`

Blocks until the record is acknowledged by the server and returns the offset.
Can only be called once - subsequent calls return the cached result.

**Example:**
```go
ack, _ := stream.IngestRecord(data)
offset, err := ack.Await()
if err != nil {
    log.Printf("Record failed: %v", err)
}
```

#### `TryGet() (int64, error, bool)`

Non-blocking check for acknowledgment status.

Returns:
- `(offset, nil, true)` if acknowledgment is ready
- `(0, nil, false)` if still pending
- `(0, error, true)` if an error occurred

**Example:**
```go
ack, _ := stream.IngestRecord(data)
// Do other work...
if offset, err, ready := ack.TryGet(); ready {
    if err != nil {
        log.Printf("Record failed: %v", err)
    } else {
        log.Printf("Record acknowledged at offset %d", offset)
    }
} else {
    log.Println("Still waiting for acknowledgment")
}
```

### `HeadersProvider`

Interface for providing custom authentication headers.

```go
type HeadersProvider interface {
    // GetHeaders returns authentication headers.
    // Called by the SDK when authentication is needed.
    GetHeaders() (map[string]string, error)
}
```

**Example implementation:**
```go
type CustomProvider struct{}

func (p *CustomProvider) GetHeaders() (map[string]string, error) {
    return map[string]string{
        "authorization": "Bearer token",
        "x-databricks-zerobus-table-name": "catalog.schema.table",
    }, nil
}
```

### `TableProperties`

```go
type TableProperties struct {
    TableName       string
    DescriptorProto []byte
}
```

### `StreamConfigurationOptions`

See [Configuration Options](#configuration-options) for details.

### `ZerobusError`

```go
type ZerobusError struct {
    Message     string
    IsRetryable bool
}
```

#### `Error() string`

Returns the error message.

#### `Retryable() bool`

Returns `true` if the error can be automatically recovered by the SDK.

## Building from Source

For contributors or those who want to build and test the SDK:

```bash
git clone https://github.com/databricks/zerobus-go-sdk.git
cd zerobus-go-sdk
make build
```

**Build specific components:**

```bash
# Build only Rust FFI
make build-rust

# Build only Go SDK
make build-go

# Build examples
make examples

# Run tests
make test

# Format code
make fmt

# Run linters
make lint
```

## Community and Contributing

This is an open source project. We welcome contributions, feedback, and bug reports.

- **[Contributing Guide](CONTRIBUTING.md)**: Learn how to contribute, including our development process and coding style
- **[Changelog](CHANGELOG.md)**: See the history of changes in the SDK
- **[Security Policy](SECURITY.md)**: Read about our security process and how to report vulnerabilities
- **[Developer Certificate of Origin (DCO)](DCO)**: Understand the agreement for contributions
- **[Open Source Attributions](NOTICE)**: See a list of the open source libraries we use

## License

This SDK is licensed under the Databricks License. See the [LICENSE](LICENSE) file for the full license text. The license is also available online at [https://www.databricks.com/legal/db-license](https://www.databricks.com/legal/db-license).

## Requirements

- **Go** 1.19 or higher
- **Rust** 1.70 or higher (for building from source)
- **Databricks** workspace with Zerobus access enabled
- **OAuth 2.0** client credentials (client ID and secret)
- **Unity Catalog** endpoint access
- **CGO** enabled (default on most systems)

---

For issues, questions, or contributions, please visit the [GitHub repository](https://github.com/databricks/zerobus-go-sdk).
