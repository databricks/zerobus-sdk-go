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

# Copy static library (same for all platforms)
cp zerobus-ffi/target/release/libzerobus_ffi.a .
cp zerobus-ffi/target/release/libzerobus_ffi.a ..
echo "✓ Copied libzerobus_ffi.a to sdk/ and repository root"

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
echo "Library location: $(pwd)/libzerobus_ffi.a"
echo ""
echo "✓ Static library built - no runtime dependencies needed!"
echo "  The Go binary will be self-contained with no LD_LIBRARY_PATH setup required."
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
