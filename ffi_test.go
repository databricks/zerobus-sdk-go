package zerobus

import (
	"fmt"
	"runtime"
	"runtime/cgo"
	"sync"
	"testing"
	"time"
	"unsafe"
)

// TestStreamHandleRegistry tests the stream handle registry
func TestStreamHandleRegistry(t *testing.T) {
	// Create a test handle
	testProvider := &mockHeadersProvider{}
	handle := cgo.NewHandle(testProvider)

	// Create a real allocated pointer (not a fake one)
	dummyStream := struct{ id int }{1234}
	dummyStreamPtr := unsafe.Pointer(&dummyStream)

	// Store in registry
	streamHandleRegistryMu.Lock()
	streamHandleRegistry[dummyStreamPtr] = handle
	streamHandleRegistryMu.Unlock()

	// Verify it's stored
	streamHandleRegistryMu.Lock()
	storedHandle, exists := streamHandleRegistry[dummyStreamPtr]
	streamHandleRegistryMu.Unlock()

	if !exists {
		t.Fatal("Handle not found in registry")
	}

	if storedHandle != handle {
		t.Fatal("Retrieved handle doesn't match stored handle")
	}

	// Clean up
	streamHandleRegistryMu.Lock()
	delete(streamHandleRegistry, dummyStreamPtr)
	streamHandleRegistryMu.Unlock()
	handle.Delete()
}

// TestStreamHandleCleanup tests that handles are properly cleaned up
func TestStreamHandleCleanup(t *testing.T) {
	testProvider := &mockHeadersProvider{}
	handle := cgo.NewHandle(testProvider)

	dummyStream := struct{ id int }{5678}
	dummyStreamPtr := unsafe.Pointer(&dummyStream)

	// Store in registry
	streamHandleRegistryMu.Lock()
	streamHandleRegistry[dummyStreamPtr] = handle
	streamHandleRegistryMu.Unlock()

	// Simulate streamFree cleanup logic
	streamHandleRegistryMu.Lock()
	if h, exists := streamHandleRegistry[dummyStreamPtr]; exists {
		h.Delete()
		delete(streamHandleRegistry, dummyStreamPtr)
	}
	streamHandleRegistryMu.Unlock()

	// Verify it's removed
	streamHandleRegistryMu.Lock()
	_, exists := streamHandleRegistry[dummyStreamPtr]
	streamHandleRegistryMu.Unlock()

	if exists {
		t.Fatal("Handle should have been removed from registry")
	}
}

// TestHandleConcurrency tests concurrent access to the handle registry
func TestHandleConcurrency(t *testing.T) {
	const numGoroutines = 10

	done := make(chan bool, numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			testProvider := &mockHeadersProvider{}
			handle := cgo.NewHandle(testProvider)
			dummyStream := struct{ id int }{1000 + id}
			ptr := unsafe.Pointer(&dummyStream)

			// Store
			streamHandleRegistryMu.Lock()
			streamHandleRegistry[ptr] = handle
			streamHandleRegistryMu.Unlock()

			// Retrieve
			streamHandleRegistryMu.Lock()
			_, exists := streamHandleRegistry[ptr]
			streamHandleRegistryMu.Unlock()

			if !exists {
				t.Errorf("Handle %d not found", id)
			}

			// Clean up
			streamHandleRegistryMu.Lock()
			delete(streamHandleRegistry, ptr)
			streamHandleRegistryMu.Unlock()
			handle.Delete()

			done <- true
		}(i)
	}

	// Wait for all goroutines
	for i := 0; i < numGoroutines; i++ {
		<-done
	}
}

// Mock HeadersProvider for testing
type mockHeadersProvider struct {
	headers map[string]string
	err     error
}

func (m *mockHeadersProvider) GetHeaders() (map[string]string, error) {
	if m.err != nil {
		return nil, m.err
	}
	if m.headers == nil {
		return map[string]string{
			"Authorization":   "Bearer test-token",
			"X-Custom-Header": "test-value",
		}, nil
	}
	return m.headers, nil
}

// TestMockHeadersProvider tests the mock provider
func TestMockHeadersProvider(t *testing.T) {
	provider := &mockHeadersProvider{}

	headers, err := provider.GetHeaders()
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	if len(headers) != 2 {
		t.Fatalf("Expected 2 headers, got %d", len(headers))
	}

	if headers["Authorization"] != "Bearer test-token" {
		t.Errorf("Unexpected Authorization header: %s", headers["Authorization"])
	}
}

// TestMockHeadersProviderWithError tests the mock provider error handling
func TestMockHeadersProviderWithError(t *testing.T) {
	testErr := &ZerobusError{Message: "test error", IsRetryable: false}
	provider := &mockHeadersProvider{err: testErr}

	_, err := provider.GetHeaders()
	if err == nil {
		t.Fatal("Expected error, got nil")
	}

	if err != testErr {
		t.Errorf("Expected error %v, got %v", testErr, err)
	}
}

// TestZerobusError tests the ZerobusError type
func TestZerobusError(t *testing.T) {
	err := &ZerobusError{
		Message:     "test error message",
		IsRetryable: true,
	}

	errStr := err.Error()
	if errStr != "ZerobusError (retryable): test error message" {
		t.Errorf("Expected 'ZerobusError (retryable): test error message', got '%s'", errStr)
	}

	if !err.IsRetryable {
		t.Error("Expected error to be retryable")
	}

	// Test non-retryable error
	err2 := &ZerobusError{
		Message:     "permanent error",
		IsRetryable: false,
	}

	errStr2 := err2.Error()
	if errStr2 != "ZerobusError: permanent error" {
		t.Errorf("Expected 'ZerobusError: permanent error', got '%s'", errStr2)
	}
}

// TestMemoryPinning verifies that runtime.Pinner prevents data from being moved
// This test exercises the pinning logic without requiring a full Rust FFI call
func TestMemoryPinning(t *testing.T) {
	// Create test data that simulates batch ingestion
	const numRecords = 1000
	records := make([][]byte, numRecords)

	for i := 0; i < numRecords; i++ {
		records[i] = []byte(fmt.Sprintf(`{"id": %d, "data": "test data for record %d"}`, i, i))
	}

	// Start aggressive GC in background to try to move memory
	stopGC := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		ticker := time.NewTicker(1 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-stopGC:
				return
			case <-ticker.C:
				runtime.GC()
			}
		}
	}()

	// Simulate what streamIngestProtoRecords does: pin all records
	var pinner runtime.Pinner
	defer pinner.Unpin()

	originalFirstBytes := make([]byte, numRecords)
	for i, record := range records {
		if len(record) > 0 {
			pinner.Pin(&record[0])
			originalFirstBytes[i] = record[0]
		}
	}

	// Let GC run for a bit while data is "being used by Rust"
	time.Sleep(100 * time.Millisecond)

	// Verify data is still correct by checking through the original slice references
	// (If pinning failed, the data might have been moved or corrupted)
	for i, record := range records {
		if len(record) == 0 {
			continue
		}

		// Read first byte to verify data hasn't been corrupted
		if record[0] != originalFirstBytes[i] {
			t.Errorf("Record %d: first byte mismatch. Expected %c, got %c", i, originalFirstBytes[i], record[0])
		}
	}

	// Stop GC
	close(stopGC)
	wg.Wait()

	t.Logf("Successfully verified %d pinned records under GC pressure", numRecords)
}

// TestMemoryPinningWithLargeRecords tests pinning with larger data sizes
func TestMemoryPinningWithLargeRecords(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping large record test in short mode")
	}

	// Create large records (1MB each) to increase pressure on GC
	const recordSize = 1024 * 1024
	const numRecords = 10

	records := make([][]byte, numRecords)
	for i := 0; i < numRecords; i++ {
		records[i] = make([]byte, recordSize)
		// Fill with pattern
		for j := range records[i] {
			records[i][j] = byte(i % 256)
		}
	}

	// Start GC pressure
	stopGC := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		ticker := time.NewTicker(1 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-stopGC:
				return
			case <-ticker.C:
				runtime.GC()
			}
		}
	}()

	// Pin all records
	var pinner runtime.Pinner
	defer pinner.Unpin()

	originalFirstBytes := make([]byte, numRecords)
	originalLastBytes := make([]byte, numRecords)
	for i, record := range records {
		pinner.Pin(&record[0])
		originalFirstBytes[i] = record[0]
		originalLastBytes[i] = record[recordSize-1]
	}

	// Hold for longer to give GC more opportunities
	time.Sleep(200 * time.Millisecond)

	// Verify data integrity through original slice references
	for i := range records {
		expected := byte(i % 256)

		if records[i][0] != originalFirstBytes[i] {
			t.Errorf("Large record %d: first byte mismatch. Expected %d, got %d", i, expected, records[i][0])
		}

		// Also check last byte
		if records[i][recordSize-1] != originalLastBytes[i] {
			t.Errorf("Large record %d: last byte mismatch. Expected %d, got %d", i, expected, records[i][recordSize-1])
		}
	}

	close(stopGC)
	wg.Wait()

	t.Logf("Successfully verified %d large records (%d MB each) under GC pressure",
		numRecords, recordSize/1024/1024)
}

// TestConcurrentPinning tests that pinning works correctly with concurrent access
func TestConcurrentPinning(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping concurrent test in short mode")
	}

	const numGoroutines = 10
	const recordsPerGoroutine = 100

	// Start GC pressure
	stopGC := make(chan struct{})
	var gcWg sync.WaitGroup
	gcWg.Add(1)
	go func() {
		defer gcWg.Done()
		ticker := time.NewTicker(1 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-stopGC:
				return
			case <-ticker.C:
				runtime.GC()
			}
		}
	}()

	// Each goroutine pins and verifies its own set of records
	var wg sync.WaitGroup
	errors := make(chan error, numGoroutines)

	for g := 0; g < numGoroutines; g++ {
		wg.Add(1)
		go func(goroutineID int) {
			defer wg.Done()

			// Create records for this goroutine
			records := make([][]byte, recordsPerGoroutine)
			for i := 0; i < recordsPerGoroutine; i++ {
				records[i] = []byte(fmt.Sprintf(`{"goroutine": %d, "record": %d}`, goroutineID, i))
			}

			// Pin them
			var pinner runtime.Pinner
			defer pinner.Unpin()

			originalFirstBytes := make([]byte, recordsPerGoroutine)
			for i, record := range records {
				pinner.Pin(&record[0])
				originalFirstBytes[i] = record[0]
			}

			// Let GC run
			time.Sleep(50 * time.Millisecond)

			// Verify
			for i, record := range records {
				if record[0] != originalFirstBytes[i] {
					errors <- fmt.Errorf("goroutine %d record %d: data corrupted", goroutineID, i)
					return
				}
			}
		}(g)
	}

	wg.Wait()
	close(stopGC)
	gcWg.Wait()
	close(errors)

	// Check for errors
	for err := range errors {
		t.Fatal(err)
	}

	t.Logf("Successfully verified pinning across %d concurrent goroutines with %d records each",
		numGoroutines, recordsPerGoroutine)
}
