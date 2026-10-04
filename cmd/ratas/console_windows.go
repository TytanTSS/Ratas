package main

import (
	"log"
	"os"

	"golang.org/x/sys/windows"
)

var (
	kernel32             = windows.NewLazySystemDLL("kernel32.dll")
	procAllocConsole     = kernel32.NewProc("AllocConsole")
	procGetConsoleWindow = kernel32.NewProc("GetConsoleWindow")
)

// ensureConsole gives the process a console again. Ebitengine frees the
// console at startup when ratas.exe was launched by double-click (it owns
// its console then), which leaves the terminal UI and server logs nowhere
// to go. Started from cmd or PowerShell the console stays and this is a no-op.
func ensureConsole() {
	if hwnd, _, _ := procGetConsoleWindow.Call(); hwnd != 0 {
		return
	}
	// Output redirected to a file or pipe: keep it there.
	if t, err := windows.GetFileType(windows.Handle(os.Stdout.Fd())); err == nil &&
		(t == windows.FILE_TYPE_DISK || t == windows.FILE_TYPE_PIPE) {
		return
	}
	if r, _, _ := procAllocConsole.Call(); r == 0 {
		return
	}
	if out, err := os.OpenFile("CONOUT$", os.O_RDWR, 0); err == nil {
		_ = windows.SetStdHandle(windows.STD_OUTPUT_HANDLE, windows.Handle(out.Fd()))
		_ = windows.SetStdHandle(windows.STD_ERROR_HANDLE, windows.Handle(out.Fd()))
		os.Stdout, os.Stderr = out, out
		log.SetOutput(out)
	}
	if in, err := os.OpenFile("CONIN$", os.O_RDWR, 0); err == nil {
		_ = windows.SetStdHandle(windows.STD_INPUT_HANDLE, windows.Handle(in.Fd()))
		os.Stdin = in
	}
}
