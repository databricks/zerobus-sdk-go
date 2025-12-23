package zerobus

import (
	"sync"
)

// RecordAck represents a pending acknowledgment for an ingested record.
// It allows ingestion with deferred acknowledgment handling.
type RecordAck struct {
	ackID  uint64
	once   sync.Once
	offset int64
	err    error
}

// Await blocks until the record is acknowledged by the server and returns the offset.
// This method can only be called once. Subsequent calls return the cached result.
//
// Example:
//
//	ack, _ := stream.IngestRecord(data)
//	// Do other work...
//	offset, err := ack.Await()
func (a *RecordAck) Await() (int64, error) {
	a.once.Do(func() {
		a.offset, a.err = streamAwaitAck(a.ackID)
	})
	return a.offset, a.err
}

// TryGet attempts to get the acknowledgment without blocking.
// Returns (offset, nil, true) if the acknowledgment is ready.
// Returns (0, nil, false) if still pending.
// Returns (0, error, true) if there was an error.
func (a *RecordAck) TryGet() (int64, error, bool) {
	return streamTryGetAck(a.ackID)
}
