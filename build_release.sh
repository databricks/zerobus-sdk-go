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

# Copy platform-specific library to release root (needed for linking)
if [ "$OS" = "darwin" ]; then
    cp sdk/zerobus-ffi/target/release/libzerobus_ffi.dylib "${RELEASE_DIR}/"
    echo "✓ Copied libzerobus_ffi.dylib (macOS)"
else
    cp sdk/zerobus-ffi/target/release/libzerobus_ffi.so "${RELEASE_DIR}/"
    echo "✓ Copied libzerobus_ffi.so (Linux)"
fi

# Create setup script
cat > "${RELEASE_DIR}/setup.sh" << 'EOF'
#!/bin/bash
# Source this file to set up the environment:
#   source setup.sh

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Set library path based on OS
if [[ "$OSTYPE" == "darwin"* ]]; then
    export DYLD_LIBRARY_PATH="${SDK_DIR}:${DYLD_LIBRARY_PATH}"
    echo "✓ Set DYLD_LIBRARY_PATH=${SDK_DIR}"
else
    export LD_LIBRARY_PATH="${SDK_DIR}:${LD_LIBRARY_PATH}"
    echo "✓ Set LD_LIBRARY_PATH=${SDK_DIR}"
fi

echo ""
echo "Zerobus Go SDK environment configured!"
echo ""
echo "Next steps:"
echo "  1. Set your Databricks credentials (see INSTALL.md)"
echo "  2. cd examples && go run basic_json_usage.go"
echo ""
EOF
chmod +x "${RELEASE_DIR}/setup.sh"

# Create installation instructions
cat > "${RELEASE_DIR}/INSTALL.md" << 'EOF'
# Installation Instructions

## Quick Start

1. Extract this archive:
   ```bash
   tar -xzf zerobus-go-sdk-*.tar.gz
   cd zerobus-go-sdk-*
   ```

2. Set up the environment (easy way):
   ```bash
   source setup.sh
   ```

   This automatically sets the library path for you!

   **Or manually:**

   **macOS:**
   ```bash
   export DYLD_LIBRARY_PATH=$(pwd):$DYLD_LIBRARY_PATH
   ```

   **Linux:**
   ```bash
   export LD_LIBRARY_PATH=$(pwd):$LD_LIBRARY_PATH
   ```

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
replace github.com/databricks/zerobus-go-sdk => /path/to/zerobus-go-sdk-darwin-arm64-v0.1.0/sdk
```

**Note:** Update the path to point to where you extracted this SDK (include the /sdk subdirectory).

### Step 3: Import in Your Code
```go
import zerobus "github.com/databricks/zerobus-go-sdk"

sdk, err := zerobus.NewZerobusSdk(endpoint, catalogURL)
```

### Step 4: Set Library Path
Before running your app, you must source the setup script:
```bash
source /path/to/zerobus-go-sdk-darwin-arm64-v0.1.0/setup.sh
```

This sets the library path so your app can find `libzerobus_ffi.dylib` (or `.so`).

## Running Examples

```bash
cd examples
export ZEROBUS_SERVER_ENDPOINT="https://your-zerobus-endpoint.databricks.com"
export DATABRICKS_WORKSPACE_URL="https://your-workspace.databricks.com"
export DATABRICKS_CLIENT_ID="your-client-id"
export DATABRICKS_CLIENT_SECRET="your-client-secret"
export ZEROBUS_TABLE_NAME="catalog.schema.table"
go run basic_json_usage.go
```

## System-Wide Installation (Optional)

To install the library system-wide:

**macOS:**
```bash
sudo cp libzerobus_ffi.dylib /usr/local/lib/
```

**Linux:**
```bash
sudo cp libzerobus_ffi.so /usr/local/lib/
sudo ldconfig
```
EOF

# Create archive
echo "Step 3: Creating archive..."
cd releases
tar -czf "${RELEASE_NAME}.tar.gz" "${RELEASE_NAME}"
cd ..

echo ""
echo "========================================="
echo "✓ Release package created successfully!"
echo "========================================="
echo ""
echo "Package: releases/${RELEASE_NAME}.tar.gz"
echo "Size: $(du -h releases/${RELEASE_NAME}.tar.gz | cut -f1)"
echo ""
echo "Users can:"
echo "  1. Download ${RELEASE_NAME}.tar.gz"
echo "  2. Extract: tar -xzf ${RELEASE_NAME}.tar.gz"
echo "  3. Follow INSTALL.md instructions"
echo ""
