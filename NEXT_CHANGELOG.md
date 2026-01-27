# NEXT CHANGELOG

## Release v0.2.0

### New Features and Improvements

- Introduced simplified `IngestRecordOffset()` API that returns offsets directly as `(int64, error)` instead of returning a future-like `RecordAck`. This is now the recommended way to ingest records.
- Batch ingestion API `IngestRecordsOffset()` that accepts multiple records and returns one offset for the entire batch. Optimized for high-throughput scenarios where ingesting multiple records at once improves performance.
- Explicit control over waiting for server acknowledgments with `WaitForOffset()` method. Allows waiting for specific offsets without blocking on all records.
- Enabled retrieval of all records that have not been acknowledged, in case of stream failure, with `GetUnackedRecords()` method.
- Enabled Rust core tracing logs visible from Go applications via `RUST_LOG` environment variable. Provides detailed debugging information from the underlying SDK.
- Updated to `databricks-zerobus-ingest-sdk` v0.4.0 with latest improvements and bug fixes

### Deprecations

- `IngestRecord()` method is deprecated in favor of `IngestRecordOffset()`. The old API remains functional for backwards compatibility but will be removed in a future major version. IDEs will show deprecation warnings with migration guidance.

### Bug Fixes

- Fixed memory leaks caused by ACK tracking futures not being properly cleaned up when streams closed
- Corrected offset values to start from 0 (matching Rust SDK's `OffsetIdGenerator` behavior) instead of 1
- Fixed stream cleanup to properly free all resources without requiring manual ACK task abortion

### Documentation

- Updated READMEs
- Reorganized examples into `json/single`, `json/batch`, `proto/single`, and `proto/batch` directories
- Added batch ingestion examples demonstrating `IngestRecordsOffset()` in both JSON and protobuf examples

### Internal Changes

- Removed `ACK_REGISTRY`, `ACK_COUNTER`, and `STREAM_ACKS` global static state from FFI layer
- Removed async task spawning and future tracking in FFI layer
- Changed internal implementation to call Rust SDK's `ingest_record_offset()` and `ingest_records_offset()` instead of deprecated APIs

### API Changes

- `IngestRecordOffset(payload interface{}) (int64, error)` - Returns offset directly after queuing record for ingestion
- `IngestRecordsOffset(records []interface{}) (int64, error)` - Batch ingestion API that returns one offset for the entire batch
- `WaitForOffset(offset int64) error` - Explicitly wait for server acknowledgment of a specific offset
- `GetUnackedRecords() ([]interface{}, error)` - Retrieve all unacknowledged records that are still in-flight (call only after stream closes/fails)
- `IngestRecord()` now returns immediately with an offset wrapped in `RecordAck`
- `RecordAck.Await()` now blocks and waits for server acknowledgment (calls Rust SDK's `wait_for_offset()`)
- `RecordAck.Offset()` returns the offset immediately without waiting
