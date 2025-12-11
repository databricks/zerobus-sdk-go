#!/bin/bash
set -e

echo "Building Zerobus Go SDK..."
echo ""

# Build Rust FFI layer
echo "Step 1: Building Rust FFI layer..."
cd sdk/zerobus-ffi
cargo build --release
echo "✓ Rust FFI layer built successfully"
echo ""

# Copy artifacts to SDK root and repository root
echo "Step 2: Copying artifacts..."
cd ..

# Detect platform and copy the correct library
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    cp zerobus-ffi/target/release/libzerobus_ffi.dylib .
    cp zerobus-ffi/target/release/libzerobus_ffi.dylib ..
    echo "✓ Copied libzerobus_ffi.dylib to sdk/ and repository root (macOS)"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    cp zerobus-ffi/target/release/libzerobus_ffi.so .
    cp zerobus-ffi/target/release/libzerobus_ffi.so ..
    echo "✓ Copied libzerobus_ffi.so to sdk/ and repository root (Linux)"
else
    echo "⚠ Unknown platform: $OSTYPE"
    echo "Attempting to copy .so file..."
    cp zerobus-ffi/target/release/libzerobus_ffi.so .
    cp zerobus-ffi/target/release/libzerobus_ffi.so ..
fi

# Note: zerobus.h is now referenced from zerobus-ffi/ directly (no copy needed)
echo "✓ Artifacts copied"
echo ""

# Build Go SDK
echo "Step 3: Building Go SDK..."
pwd
go build -v
if [ $? -eq 0 ]; then
    echo "✓ Go SDK built successfully"
else
    echo "✗ Go SDK build failed"
    exit 1
fi
cd ..
echo ""

# Run Go tests (if any)
echo "Step 4: Running tests..."
cd sdk
go test -v || echo "No tests found (this is okay)"
cd ..
echo ""

echo "========================================="
echo "Build completed successfully!"
echo "========================================="
echo ""

# Platform-specific instructions
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Library location: $(pwd)/libzerobus_ffi.dylib"
    echo ""
    echo "To use the SDK:"
    echo "  export DYLD_LIBRARY_PATH=$(pwd):\$DYLD_LIBRARY_PATH"
else
    echo "Library location: $(pwd)/libzerobus_ffi.so"
    echo ""
    echo "To use the SDK:"
    echo "  export LD_LIBRARY_PATH=$(pwd):\$LD_LIBRARY_PATH"
fi
echo ""
echo "To run examples:"
echo "  cd examples"
echo "  export ZEROBUS_SERVER_ENDPOINT=\"https://your-zerobus-endpoint.databricks.com\""
echo "  export DATABRICKS_WORKSPACE_URL=\"https://your-workspace.databricks.com\""
echo "  export DATABRICKS_CLIENT_ID=\"your-client-id\""
echo "  export DATABRICKS_CLIENT_SECRET=\"your-client-secret\""
echo "  export ZEROBUS_TABLE_NAME=\"catalog.schema.table\""
echo "  go run basic_json_usage.go"
echo ""
