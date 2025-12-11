# Examples

Two examples showing JSON and Protocol Buffer ingestion:

## 1. JSON (Simple)

```bash
export ZEROBUS_SERVER_ENDPOINT="..."
export DATABRICKS_WORKSPACE_URL="..."
export DATABRICKS_CLIENT_ID="..."
export DATABRICKS_CLIENT_SECRET="..."
export ZEROBUS_TABLE_NAME="catalog.schema.table"

go run basic_json_usage.go
```

## 2. Protocol Buffers (Advanced)

Generate code first:
```bash
./generate_proto.sh
go run basic_proto_usage.go
```

## Protocol Buffers Details

### Step 1: Define Your Proto File

Create a `.proto` file (e.g., `air_quality.proto`):

```protobuf
syntax = "proto2";

package examples;

option go_package = "github.com/databricks/zerobus-go-sdk/examples/pb";

message AirQuality {
    optional string device_name = 1;
    optional int32 temp = 2;
    optional int64 humidity = 3;
}
```

### Step 2: Install Protocol Buffer Compiler

```bash
# Ubuntu/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf

# Install Go plugin
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
```

### Step 3: Generate Go Code

```bash
cd examples
./generate_proto.sh
```

Or manually:

```bash
mkdir -p pb
protoc --go_out=. --go_opt=paths=source_relative air_quality.proto
```

This creates `pb/air_quality.pb.go` with:
- Go struct: `pb.AirQuality`
- Serialization methods
- File descriptor: `pb.File_air_quality_proto`

### Step 4: Use in Your Code

```go
import (
    zerobus "github.com/databricks/zerobus-go-sdk"
    pb "github.com/databricks/zerobus-go-sdk/examples/pb"
    "google.golang.org/protobuf/proto"
)

// Create message
message := &pb.AirQuality{
    DeviceName: proto.String("sensor-001"),
    Temp:       proto.Int32(25),
    Humidity:   proto.Int64(65),
}

// Marshal to bytes
data, _ := proto.Marshal(message)

// Ingest
offset, _ := stream.IngestProtoRecord(data)
```

### Step 5: Run the Protobuf Example

```bash
# Set environment variables
export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.table"

# Run
go run basic_proto_usage.go
```

## Dependencies

The examples require:

```bash
go get google.golang.org/protobuf/proto
go get google.golang.org/protobuf/reflect/protodesc
go get google.golang.org/protobuf/types/descriptorpb
```

These are already specified in `go.mod`.
