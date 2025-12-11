# Tests

This directory is reserved for integration tests and end-to-end tests.

## Structure

```
tests/
├── README.md           # This file
├── integration/        # Integration tests (coming soon)
└── e2e/               # End-to-end tests (coming soon)
```

## Running Tests

### Unit Tests

Unit tests are located alongside the source code in the `sdk/` directory:

```bash
cd sdk
go test -v ./...
```

### Integration Tests

Integration tests will test the Go SDK against the Rust FFI layer:

```bash
make test
```

### End-to-End Tests

E2E tests will test the complete workflow including authentication and actual data ingestion against a test Databricks environment:

```bash
# Coming soon
```

## Contributing Tests

When adding new features, please include:

1. **Unit tests** in the same package as your code
2. **Integration tests** if your feature spans multiple components
3. **Examples** in the `examples/` directory for user-facing features

See [CONTRIBUTING.md](../CONTRIBUTING.md) for more details.
