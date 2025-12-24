# Zerobus Go SDK Examples

This directory contains examples demonstrating how to use the Zerobus Go SDK to ingest data into Databricks Delta tables.

## Available Examples

### 1. JSON Example
**Recommended for getting started** - A simpler example that uses JSON for data serialization.

- Location: `examples/basic_example_json/basic_json_usage.go`
- No schema generation required
- Direct JSON string ingestion
- Easier to understand and modify
- Great for quick prototyping

### 2. Protocol Buffers Example
A more advanced example that uses Protocol Buffers for type-safe data serialization.

- Location: `examples/basic_example_proto/basic_proto_usage.go`
- Requires protobuf schema generation
- Type-safe record creation
- Better for production use cases
- More efficient binary encoding

## Common Features

Both examples demonstrate:
- Creating a stream with OAuth authentication
- Ingesting records asynchronously
- Awaiting acknowledgments
- Properly closing the stream
- Configuring credentials and endpoints

---

## Prerequisites

### 1. Create a Databricks Table

First, create a table in your Databricks workspace using the following SQL:

```sql
CREATE TABLE catalog.schema.air_quality (
  device_name STRING,
  temp INT,
  humidity BIGINT
);
```

Replace `catalog.schema.air_quality` with your actual catalog, schema, and table name.

**Note:** This schema matches the examples. You can modify it for your use case, but make sure to update the example code accordingly.

### 2. Set Up OAuth Service Principal

You'll need a Databricks service principal with OAuth credentials:

1. In your Databricks workspace, go to **Settings** → **Identity and Access**
2. Create a service principal or use an existing one
3. Generate OAuth credentials (client ID and secret)
4. Grant the service principal the following permissions on your table:
   - `SELECT` - Read table schema
   - `MODIFY` - Write data to the table
   - `USE CATALOG` and `USE SCHEMA` - Access catalog and schema

### 3. Configure Credentials

Both examples require the same environment variables. You'll need to set these before running:

```bash
export ZEROBUS_SERVER_ENDPOINT="https://workspace-id.zerobus.region.cloud.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.cloud.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.air_quality"
```

**How to get these values:**

- **ZEROBUS_SERVER_ENDPOINT** - Zerobus ingestion endpoint for your workspace
  - **AWS**: `https://<workspace-id>.zerobus.<region>.cloud.databricks.com`
  - **Azure**: `https://<workspace-id>.zerobus.<region>.azuredatabricks.net`
  - Ask your Databricks account team for the correct endpoint

- **DATABRICKS_WORKSPACE_URL** - Your Databricks workspace URL (Unity Catalog endpoint)
  - This is the URL you use to access your Databricks workspace
  - **AWS**: `https://<workspace>.cloud.databricks.com`
  - **Azure**: `https://<workspace>.azuredatabricks.net`

- **DATABRICKS_CLIENT_ID** - OAuth 2.0 client ID from your service principal
  - Found in Settings → Identity and Access → Service Principals
  - Example: `a1b2c3d4-e5f6-7890-abcd-ef1234567890`

- **DATABRICKS_CLIENT_SECRET** - OAuth 2.0 client secret from your service principal
  - Generated when creating OAuth credentials for the service principal
  - This is only shown once, so save it securely

- **ZEROBUS_TABLE_NAME** - Full table name in format `catalog.schema.table`
  - Example: `main.ingestion.air_quality`

---

## Running the JSON Example

The JSON example is simpler and doesn't require schema generation.

### Quick Start

```bash
# 1. Set credentials
export ZEROBUS_SERVER_ENDPOINT="https://workspace-id.zerobus.region.cloud.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.cloud.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.air_quality"

# 2. Run the example
cd examples/basic_example_json
go run basic_json_usage.go
```

### Expected Output

```
Ingesting records ...
Queued record 0 (awaiting acknowledgment...)
Queued record 1 (awaiting acknowledgment...)
Queued record 2 (awaiting acknowledgment...)
Queued record 3 (awaiting acknowledgment...)
Queued record 4 (awaiting acknowledgment...)
Flushing stream...
All records successfully ingested and acknowledged!
```

### Code Highlights

The JSON example uses string-based JSON records:

```go
jsonRecord := `{
    "device_name": "sensor-001",
    "temp": 20,
    "humidity": 60
}`

// Ingest asynchronously - returns immediately
ack, err := stream.IngestRecord(jsonRecord)
if err != nil {
    log.Printf("Failed to ingest: %v", err)
}

// Await acknowledgment later
offset, err := ack.Await()
```

Key features:
- Set `RecordType = RecordTypeJson` in `StreamConfigurationOptions`
- No descriptor file needed
- Pass JSON strings directly to `IngestRecord()`
- Records are queued immediately without blocking

---

## Running the Protocol Buffers Example

The Protocol Buffers example provides type safety and better performance.

### Step 1: Install Protocol Buffer Compiler

```bash
# Ubuntu/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf

# Install Go plugin
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
```

### Step 2: Generate Go Code from Proto Schema

The example includes `air_quality.proto`. Generate the Go code:

```bash
cd examples/basic_example_proto
./generate_proto.sh
```

This creates:
- `pb/air_quality.pb.go` - Generated Go structs
- File descriptor for the table schema

**Or generate manually:**
```bash
mkdir -p pb
protoc --go_out=. --go_opt=paths=source_relative air_quality.proto
```

### Step 3: Set Credentials

```bash
export ZEROBUS_SERVER_ENDPOINT="https://workspace-id.zerobus.region.cloud.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.cloud.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.air_quality"
```

### Step 4: Run the Example

```bash
cd examples/basic_example_proto
go run basic_proto_usage.go
```

### Expected Output

```
Ingesting records ...
Queued record 0 (temp=20, humidity=60)
Queued record 1 (temp=21, humidity=61)
...
Flushing stream...
All records successfully ingested!
```

### Code Highlights

The Protocol Buffers example uses strongly-typed structs:

```go
import (
    "google.golang.org/protobuf/proto"
    "zerobus-examples/pb"
)

// Create typed message
message := &pb.AirQuality{
    DeviceName: proto.String("sensor-001"),
    Temp:       proto.Int32(25),
    Humidity:   proto.Int64(65),
}

// Marshal to bytes
data, err := proto.Marshal(message)

// Ingest asynchronously
ack, err := stream.IngestRecord(data)

// Await acknowledgment
offset, err := ack.Await()
```

Key features:
- Type-safe record creation with compile-time checks
- Efficient binary encoding via Protocol Buffers
- Requires descriptor file and generated Go structs
- Set `RecordType = RecordTypeProto` (this is the default)

---

## Adapting for Your Custom Table

### For JSON Example

Simply modify the JSON string in `basic_json_usage.go` to match your table's schema:

```go
jsonRecord := `{
    "your_field_1": "value1",
    "your_field_2": 123,
    "your_field_3": true
}`
```

**Important:** Make sure the JSON fields match your table schema exactly, including:
- Field names (case-sensitive)
- Data types (STRING, INT, BIGINT, DOUBLE, BOOLEAN, TIMESTAMP, etc.)

No schema generation needed!

### For Protocol Buffers Example

To use your own custom table, you'll need to create a proto schema and generate Go code.

**Step 1: Create Your Proto File**

Create a `.proto` file matching your table schema (e.g., `my_table.proto`):

```protobuf
syntax = "proto2";

package examples;

option go_package = "github.com/databricks/zerobus-sdk-go/examples/pb";

message MyTable {
    optional string field1 = 1;
    optional int32 field2 = 2;
    optional int64 field3 = 3;
}
```

**Step 2: Generate Go Code**

```bash
protoc --go_out=. --go_opt=paths=source_relative my_table.proto
```

**Step 3: Update Your Code**

```go
import "zerobus-examples/pb"

// Create message
message := &pb.MyTable{
    Field1: proto.String("value"),
    Field2: proto.Int32(123),
    Field3: proto.Int64(456),
}

// Marshal and ingest
data, _ := proto.Marshal(message)
ack, _ := stream.IngestRecord(data)
```

---

## Common Code Patterns

Both examples follow the same general flow:

### 1. Create SDK Instance

```go
sdk, err := zerobus.NewZerobusSdk(
    zerobusEndpoint,
    unityCatalogURL,
)
if err != nil {
    log.Fatal(err)
}
defer sdk.Free()
```

### 2. Configure Stream Options

**JSON Example:**
```go
options := zerobus.DefaultStreamConfigurationOptions()
options.MaxInflightRequests = 50000
options.RecordType = zerobus.RecordTypeJson  // Important!
```

**Protocol Buffers Example:**
```go
options := zerobus.DefaultStreamConfigurationOptions()
options.RecordType = zerobus.RecordTypeProto  // This is the default
```

### 3. Create Stream

```go
stream, err := sdk.CreateStream(
    zerobus.TableProperties{
        TableName:       tableName,
        DescriptorProto: descriptorBytes,  // nil for JSON
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

### 4. Ingest Records Asynchronously

**Fire off multiple records without waiting:**
```go
acks := make([]*zerobus.RecordAck, 0)

for i := 0; i < 100; i++ {
    ack, err := stream.IngestRecord(data)
    if err != nil {
        log.Printf("Failed: %v", err)
        continue
    }
    acks = append(acks, ack)
}
```

### 5. Await Acknowledgments

**Option A: Await all**
```go
for i, ack := range acks {
    offset, err := ack.Await()
    if err != nil {
        log.Printf("Record %d failed: %v", i, err)
    }
}
```

**Option B: Use Flush (waits for all pending)**
```go
if err := stream.Flush(); err != nil {
    log.Fatal(err)
}
```

### 6. Check Acknowledgment Status (Non-blocking)

```go
if offset, err, ready := ack.TryGet(); ready {
    if err != nil {
        log.Printf("Failed: %v", err)
    } else {
        log.Printf("Offset: %d", offset)
    }
} else {
    log.Println("Still pending...")
}
```

---

## Performance Tips

### Concurrent Streams

You can create multiple streams for parallel ingestion:

```go
var wg sync.WaitGroup
for partition := 0; partition < 4; partition++ {
    wg.Add(1)
    go func() {
        defer wg.Done()

        stream, _ := sdk.CreateStream(tableProps, clientID, clientSecret, options)
        defer stream.Close()

        // Ingest records...
    }()
}
wg.Wait()
```

---

## Choosing Between JSON and Protocol Buffers

| Feature | JSON Example | Protocol Buffers Example |
|---------|-------------|-------------------------|
| **Setup Complexity** | Simple - no schema files | Requires protoc and code generation |
| **Type Safety** | Runtime validation only | Compile-time type checking |
| **Performance** | Text-based encoding | Efficient binary encoding |
| **Flexibility** | Easy to modify on-the-fly | Requires regenerating code |
| **Best For** | Prototyping, simple use cases | Production, high-throughput scenarios |
| **Learning Curve** | Low | Moderate |

**Recommendation:** Start with the JSON example for quick prototyping, then migrate to Protocol Buffers for production deployments where type safety and performance matter.

---

## Troubleshooting

### Error: "Failed to create SDK"

**Possible causes:**
- Invalid `ZEROBUS_SERVER_ENDPOINT` or `DATABRICKS_WORKSPACE_URL`
- Network connectivity issues
- Invalid endpoint URLs

**Solution:** Verify your endpoint URLs are correct and accessible.

### Error: "Failed to create stream"

**Possible causes:**
- Invalid OAuth credentials (client ID or secret)
- Service principal lacks permissions on the table
- Table doesn't exist
- Wrong table name format

**Solution:**
1. Verify credentials are correct
2. Check service principal has SELECT and MODIFY permissions
3. Verify table exists: `SHOW TABLES IN catalog.schema`
4. Use full three-part name: `catalog.schema.table`

### Error: "Authentication failed" or "Invalid token"

**Possible causes:**
- OAuth credentials expired or invalid
- Incorrect Unity Catalog endpoint
- Service principal not properly configured

**Solution:**
1. Regenerate OAuth credentials in Databricks
2. Verify `DATABRICKS_WORKSPACE_URL` is your Unity Catalog endpoint
3. Ensure service principal has proper permissions

### Error: Schema mismatch (Protocol Buffers)

**Possible causes:**
- Proto schema doesn't match table schema
- Wrong descriptor file loaded
- Field types don't match

**Solution:**
1. Verify proto fields match table columns exactly
2. Check data type mappings (int32 → INT, int64 → BIGINT, etc.)
3. Regenerate proto file if table schema changed

### Error: JSON parsing errors (JSON example)

**Possible causes:**
- JSON structure doesn't match table schema
- Invalid JSON syntax
- Type mismatches (passing string instead of number)

**Solution:**
1. Verify JSON fields match table columns exactly (case-sensitive)
2. Check JSON is valid: `echo '{"field": "value"}' | jq`
3. Ensure data types match (numbers not quoted, strings quoted)

### Warning: CGO compilation warnings

If you see warnings like "implicit declaration of function", these are harmless CGO quirks and can be ignored. The SDK builds successfully despite these warnings.

---

## Next Steps

- Try ingesting larger batches of records
- Experiment with different `StreamConfigurationOptions`
- Add error handling and retry logic for production use
- Implement monitoring and metrics
- Use the SDK in your application

## Additional Resources

- [Main SDK Documentation](../README.md)
- [API Reference](../README.md#api-reference)
- [CHANGELOG](../CHANGELOG.md)
- [Databricks Unity Catalog Documentation](https://docs.databricks.com/unity-catalog/index.html)
- [Protocol Buffers Documentation](https://protobuf.dev/)
