#!/bin/bash
set -e

# This script is automatically invoked by `go generate` or `go build`
# It builds the Rust FFI library and places it where CGO expects it

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FFI_DIR="$SCRIPT_DIR/zerobus-ffi"
OUTPUT_DIR="$SCRIPT_DIR"

# Skip rebuild if library already exists and is newer than source
if [ -f "$OUTPUT_DIR/libzerobus_ffi.a" ]; then
    # Check if any Rust source file is newer than the library
    NEEDS_REBUILD=0
    while IFS= read -r -d '' file; do
        if [ "$file" -nt "$OUTPUT_DIR/libzerobus_ffi.a" ]; then
            NEEDS_REBUILD=1
            break
        fi
    done < <(find "$FFI_DIR/src" -name "*.rs" -print0 2>/dev/null)

    if [ $NEEDS_REBUILD -eq 0 ]; then
        echo "✓ Rust library up to date, skipping rebuild"
        exit 0
    fi
fi

echo "Building Rust FFI library..."

cd "$FFI_DIR"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Determine Rust target for Windows MinGW compatibility
if [[ "$OS" == *"mingw"* ]] || [[ "$OS" == *"msys"* ]] || [[ "$OS" == *"cygwin"* ]] || [[ -n "$MSYSTEM" ]]; then
    echo "Detected Windows/MinGW environment - building for GNU target..."
    TARGET="x86_64-pc-windows-gnu"
    cargo build --release --target "$TARGET"
elif command -v cargo-zigbuild &> /dev/null; then
    echo "Using cargo-zigbuild for optimized build..."
    cargo zigbuild --release
else
    echo "Using cargo (install cargo-zigbuild for better cross-compilation)"
    cargo build --release
fi

if [ -f "target/release/libzerobus_ffi.a" ]; then
    cp "target/release/libzerobus_ffi.a" "$OUTPUT_DIR/"
    echo "✓ Rust library built successfully: $OUTPUT_DIR/libzerobus_ffi.a"
elif [ -f "target/x86_64-pc-windows-gnu/release/libzerobus_ffi.a" ]; then
    # Windows GNU target
    cp "target/x86_64-pc-windows-gnu/release/libzerobus_ffi.a" "$OUTPUT_DIR/"
    echo "✓ Rust library built successfully: $OUTPUT_DIR/libzerobus_ffi.a (Windows GNU)"
elif [ -f "target/release/zerobus_ffi.lib" ]; then
    # Windows MSVC: copy .lib as .a for CGO compatibility
    cp "target/release/zerobus_ffi.lib" "$OUTPUT_DIR/libzerobus_ffi.a"
    echo "✓ Rust library built successfully: $OUTPUT_DIR/libzerobus_ffi.a (from zerobus_ffi.lib)"
else
    echo "✗ Error: Could not find Rust library"
    echo "   Tried: target/release/libzerobus_ffi.a"
    echo "          target/x86_64-pc-windows-gnu/release/libzerobus_ffi.a"
    echo "          target/release/zerobus_ffi.lib"
    exit 1
fi
