#!/bin/bash
set -e

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Normalize arch names
if [ "$ARCH" = "x86_64" ]; then
    ARCH="amd64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    ARCH="arm64"
fi

VERSION=${1:-"v0.1.0"}
RELEASE_NAME="zerobus-go-sdk-${OS}-${ARCH}-${VERSION}"
RELEASE_DIR="releases/${RELEASE_NAME}"

echo "Building release package: ${RELEASE_NAME}"
echo ""

# Clean and create release directory
rm -rf releases
mkdir -p "${RELEASE_DIR}"

# Build Rust FFI
echo "Step 1: Building Rust FFI..."
cd sdk/zerobus-ffi
cargo build --release
cd ../..

# Copy SDK files
echo "Step 2: Copying SDK files..."
mkdir -p "${RELEASE_DIR}/sdk"

# Copy only the Go SDK files (not the entire zerobus-ffi build directory)
cp sdk/*.go "${RELEASE_DIR}/sdk/"
cp sdk/go.mod "${RELEASE_DIR}/sdk/"

# Copy only the header file (needed for CGO compilation)
mkdir -p "${RELEASE_DIR}/sdk/zerobus-ffi"
cp sdk/zerobus-ffi/zerobus.h "${RELEASE_DIR}/sdk/zerobus-ffi/"

# Copy README and examples
cp README.md "${RELEASE_DIR}/"
cp -r examples "${RELEASE_DIR}/"

# Copy static library to release root (needed for linking)
cp sdk/zerobus-ffi/target/release/libzerobus_ffi.a "${RELEASE_DIR}/"
echo "✓ Copied libzerobus_ffi.a (static library)"

# Create quick start script
cat > "${RELEASE_DIR}/quickstart.sh" << 'EOF'
#!/bin/bash
echo "Zerobus Go SDK - Quick Start"
echo ""
echo "The SDK uses static linking - no library path configuration needed!"
echo ""
echo "Next steps:"
echo "  1. Set your Databricks credentials:"
echo "     export ZEROBUS_SERVER_ENDPOINT=\"https://your-zerobus-endpoint.databricks.com\""
echo "     export DATABRICKS_WORKSPACE_URL=\"https://your-workspace.databricks.com\""
echo "     export DATABRICKS_CLIENT_ID=\"your-client-id\""
echo "     export DATABRICKS_CLIENT_SECRET=\"your-client-secret\""
echo "     export ZEROBUS_TABLE_NAME=\"catalog.schema.table\""
echo ""
echo "  2. Run an example:"
echo "     cd examples && go run basic_json_usage.go"
echo ""
echo "  3. For detailed usage, see INSTALL.md"
echo ""
EOF
chmod +x "${RELEASE_DIR}/quickstart.sh"

# Create installation instructions
cat > "${RELEASE_DIR}/INSTALL.md" << 'EOF'
# Installation Instructions

## Quick Start

1. Extract this archive:
   ```bash
   tar -xzf zerobus-go-sdk-*.tar.gz
   cd zerobus-go-sdk-*
   ```

2. **That's it!** The SDK uses static linking, no library path configuration needed.

3. Set your credentials and run:
   ```bash
   export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
   export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
   export DATABRICKS_CLIENT_ID="your-client-id"
   export DATABRICKS_CLIENT_SECRET="your-client-secret"
   export ZEROBUS_TABLE_NAME="catalog.schema.table"

   cd examples
   go run basic_json_usage.go
   ```

## Why No Setup Script?

This SDK uses **static linking**, which means the Rust library is compiled directly into your Go binary.

## Using in Your Go Project

**Important:** This SDK is distributed as a local package (not published to a repository yet).
You must use the extracted directory with a `replace` directive in your `go.mod`.

### Step 1: Create Your Project
```bash
mkdir my-app && cd my-app
go mod init mycompany.com/my-app
```

### Step 2: Add SDK to `go.mod`
```go
require github.com/databricks/zerobus-go-sdk v0.1.0

// Point to the extracted SDK directory
replace github.com/databricks/zerobus-go-sdk => /path/to/zerobus-go-sdk-linux-amd64-v0.1.0/sdk
```

**Note:** Update the path to point to where you extracted this SDK (include the /sdk subdirectory).

### Step 3: Import in Your Code
```go
import zerobus "github.com/databricks/zerobus-go-sdk"

sdk, err := zerobus.NewZerobusSdk(endpoint, catalogURL)
```

### Step 4: Build and Run
```bash
go build -o my-app
./my-app
```

**That's it!** The binary includes everything it needs and can be deployed anywhere.

## Running Examples

```bash
cd examples
export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.table"
# Change the json string inside basic_json_usage.go to match the schema of your table
go run basic_json_usage.go
```
EOF

# Create archive
echo "Step 3: Creating archive..."
cd releases
tar -czf "${RELEASE_NAME}.tar.gz" "${RELEASE_NAME}"
cd ..

echo "Creating zip..."
cd releases
zip -r -q "${RELEASE_NAME}.zip" "${RELEASE_NAME}"
cd ..

echo ""
echo "========================================="
echo "✓ Release package created successfully"
echo "========================================="
echo ""
echo "Package: releases/${RELEASE_NAME}.tar.gz"
echo "Size: $(du -h releases/${RELEASE_NAME}.tar.gz | cut -f1)"
echo ""
echo ""
echo "Users can:"
echo "  1. Download ${RELEASE_NAME}.tar.gz"
echo "  2. Extract: tar -xzf ${RELEASE_NAME}.tar.gz"
echo "  3. Run: ./quickstart.sh"
echo "  4. Start coding - no setup required!"
echo ""
