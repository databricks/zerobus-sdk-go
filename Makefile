# Makefile for zerobus-sdk-go

.PHONY: help build build-rust build-go clean fmt fmt-go fmt-rust lint lint-go lint-rust check test test-go test-rust examples release

help:
	@echo "Available targets:"
	@echo "  make build          - Build both Rust FFI and Go SDK"
	@echo "  make build-rust     - Build only the Rust FFI layer"
	@echo "  make build-go       - Build only the Go SDK"
	@echo "  make clean          - Remove build artifacts"
	@echo "  make fmt            - Format all code (Go and Rust)"
	@echo "  make fmt-go         - Format Go code"
	@echo "  make fmt-rust       - Format Rust code"
	@echo "  make lint           - Run linters on all code"
	@echo "  make lint-go        - Run Go linters"
	@echo "  make lint-rust      - Run Rust linters"
	@echo "  make check          - Run all checks (fmt and lint)"
	@echo "  make test           - Run all tests (Rust and Go)"
	@echo "  make test-rust      - Run Rust unit tests"
	@echo "  make test-go        - Run Go unit tests"
	@echo "  make examples       - Build all examples"
	@echo "  make release        - Build release package"

build: build-rust build-go

build-rust:
	@echo "Building Rust FFI layer..."
	@# On Windows with MinGW, we need to build for the GNU target
	@if [ "$$OS" = "Windows_NT" ]; then \
		echo "Detected Windows - building for x86_64-pc-windows-gnu target"; \
		cd zerobus-ffi && cargo build --release --target x86_64-pc-windows-gnu; \
	else \
		cd zerobus-ffi && cargo build --release; \
	fi
	@echo "Copying static library and header..."
	@if [ -f zerobus-ffi/target/release/libzerobus_ffi.a ]; then \
		cp zerobus-ffi/target/release/libzerobus_ffi.a .; \
	elif [ -f zerobus-ffi/target/x86_64-pc-windows-gnu/release/libzerobus_ffi.a ]; then \
		cp zerobus-ffi/target/x86_64-pc-windows-gnu/release/libzerobus_ffi.a .; \
	elif [ -f zerobus-ffi/target/release/zerobus_ffi.lib ]; then \
		cp zerobus-ffi/target/release/zerobus_ffi.lib libzerobus_ffi.a; \
	else \
		echo "Error: Could not find Rust library"; \
		exit 1; \
	fi
	cp zerobus-ffi/zerobus.h .
	@echo "✓ Rust FFI layer built successfully"

build-go: build-rust
	@echo "Building Go SDK..."
	go build -v
	@echo "✓ Go SDK built successfully"

clean:
	@echo "Cleaning build artifacts..."
	cd zerobus-ffi && cargo clean
	rm -f libzerobus_ffi.a
	rm -rf releases
	@echo "✓ Clean complete"

fmt: fmt-go fmt-rust

fmt-go:
	@echo "Formatting Go code..."
	go fmt ./...
	cd examples/basic_example_json && go fmt ./...
	cd examples/basic_example_proto && go fmt ./...

fmt-rust:
	@echo "Formatting Rust code..."
	cd zerobus-ffi && cargo fmt --all

lint: lint-go lint-rust

lint-go:
	@echo "Linting Go code..."
	go vet ./...
	cd examples/basic_example_json && go vet ./...
	cd examples/basic_example_proto && go vet ./...

lint-rust:
	@echo "Linting Rust code..."
	cd zerobus-ffi && cargo clippy --all -- -D warnings

check: fmt lint

test: test-rust test-go

test-rust:
	@echo "Running Rust tests..."
	cd zerobus-ffi && cargo test -- --test-threads=1

test-go:
	@echo "Running Go tests..."
	go test -v

examples: build
	@echo "Building examples..."
	cd examples/basic_example_json && go build basic_json_usage.go
	cd examples/basic_example_proto && go build basic_proto_usage.go
	@echo "✓ Examples built successfully"

release:
	@echo "Building release package..."
	./scripts/build-release.sh
