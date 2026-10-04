//go:build !windows

package main

// ensureConsole is only needed on Windows, see console_windows.go.
func ensureConsole() {}
