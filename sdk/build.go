package zerobus

//go:generate bash build_rust.sh

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

// This file provides utilities for building the Rust FFI library.
// Users must run: go generate github.com/databricks/zerobus-go-sdk/sdk
// before building their application.

func init() {
	// Check if library exists and provide helpful error if not
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		return
	}
	sdkDir := filepath.Dir(filename)
	libPath := filepath.Join(sdkDir, "libzerobus_ffi.a")

	if _, err := os.Stat(libPath); os.IsNotExist(err) {
		fmt.Fprintf(os.Stderr, "\n"+
			"═══════════════════════════════════════════════════════════════\n"+
			"ERROR: Rust FFI library not found\n"+
			"═══════════════════════════════════════════════════════════════\n"+
			"\n"+
			"The Zerobus Go SDK requires a one-time build step:\n"+
			"\n"+
			"  go generate github.com/databricks/zerobus-go-sdk/sdk\n"+
			"\n"+
			"Or if you're developing locally:\n"+
			"\n"+
			"  cd sdk && go generate\n"+
			"\n"+
			"Prerequisites:\n"+
			"  - Rust 1.70+ (https://rustup.rs)\n"+
			"  - Optional: cargo-zigbuild for cross-compilation\n"+
			"\n"+
			"This builds the Rust FFI library (takes 2-5 minutes first time).\n"+
			"After that, regular 'go build' will work normally.\n"+
			"\n"+
			"═══════════════════════════════════════════════════════════════\n\n")
	}
}

// ensureRustLibrary checks if the Rust library exists and builds it if needed
func ensureRustLibrary() error {
	// Get the directory of this source file
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		return fmt.Errorf("failed to determine source directory")
	}
	sdkDir := filepath.Dir(filename)
	libPath := filepath.Join(sdkDir, "libzerobus_ffi.a")

	// Check if library already exists
	if _, err := os.Stat(libPath); err == nil {
		// Library exists, check if it needs rebuilding
		if !needsRebuild(sdkDir, libPath) {
			return nil
		}
	}

	fmt.Println("Building Rust FFI library (first time or after update)...")
	fmt.Println("This may take 2-5 minutes...")

	return buildRustLibrary(sdkDir)
}

// needsRebuild checks if any Rust source file is newer than the library
func needsRebuild(sdkDir, libPath string) bool {
	libInfo, err := os.Stat(libPath)
	if err != nil {
		return true
	}

	ffiDir := filepath.Join(sdkDir, "zerobus-ffi", "src")
	needsRebuild := false

	filepath.Walk(ffiDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if filepath.Ext(path) == ".rs" && info.ModTime().After(libInfo.ModTime()) {
			needsRebuild = true
			return filepath.SkipAll
		}
		return nil
	})

	return needsRebuild
}

// buildRustLibrary builds the Rust FFI library
func buildRustLibrary(sdkDir string) error {
	ffiDir := filepath.Join(sdkDir, "zerobus-ffi")

	// Check if Rust is installed
	if _, err := exec.LookPath("cargo"); err != nil {
		return fmt.Errorf("cargo not found. Install Rust from https://rustup.rs")
	}

	// Prefer cargo-zigbuild if available
	var cmd *exec.Cmd
	if _, err := exec.LookPath("cargo-zigbuild"); err == nil {
		fmt.Println("Using cargo-zigbuild for optimized cross-compilation...")
		cmd = exec.Command("cargo", "zigbuild", "--release")
	} else {
		fmt.Println("Using cargo (install cargo-zigbuild for better cross-compilation)...")
		cmd = exec.Command("cargo", "build", "--release")
	}

	cmd.Dir = ffiDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("cargo build failed: %w", err)
	}

	// Copy library to SDK directory
	srcLib := filepath.Join(ffiDir, "target", "release", "libzerobus_ffi.a")
	dstLib := filepath.Join(sdkDir, "libzerobus_ffi.a")

	data, err := os.ReadFile(srcLib)
	if err != nil {
		return fmt.Errorf("failed to read built library: %w", err)
	}

	if err := os.WriteFile(dstLib, data, 0644); err != nil {
		return fmt.Errorf("failed to copy library: %w", err)
	}

	fmt.Println("✓ Rust FFI library built successfully")
	return nil
}
