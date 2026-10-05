//! On Windows the release build is a window application without a console;
//! the dedicated server and the command line help get one back.

#[cfg(windows)]
pub fn ensure() {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const FILE_TYPE_DISK: u32 = 1;
    const FILE_TYPE_PIPE: u32 = 3;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> Handle;
        fn AttachConsole(pid: u32) -> i32;
        fn AllocConsole() -> i32;
        fn GetStdHandle(n: u32) -> Handle;
        fn SetStdHandle(n: u32, h: Handle) -> i32;
        fn GetFileType(h: Handle) -> u32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            sec: *mut c_void,
            disp: u32,
            flags: u32,
            tmpl: Handle,
        ) -> Handle;
    }
    unsafe {
        if !GetConsoleWindow().is_null() {
            return;
        }
        // output redirected to a file or a pipe: keep it there
        let out = GetStdHandle(STD_OUTPUT_HANDLE);
        if !out.is_null() {
            let t = GetFileType(out);
            if t == FILE_TYPE_DISK || t == FILE_TYPE_PIPE {
                return;
            }
        }
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 && AllocConsole() == 0 {
            return;
        }
        let open = |name: &str| {
            let w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            CreateFileW(
                w.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        let out = open("CONOUT$");
        if out as isize != -1 {
            SetStdHandle(STD_OUTPUT_HANDLE, out);
            SetStdHandle(STD_ERROR_HANDLE, out);
        }
        let inp = open("CONIN$");
        if inp as isize != -1 {
            SetStdHandle(STD_INPUT_HANDLE, inp);
        }
    }
}

#[cfg(not(windows))]
pub fn ensure() {}
