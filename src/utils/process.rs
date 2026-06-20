use std::path::Path;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
};

pub fn get_foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            let name = String::from_utf16_lossy(&class_name[..len as usize]);
            if name == "Progman" || name == "WorkerW" || name == "Shell_TrayWnd" {
                return None;
            }
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }

        get_process_name_by_pid(pid)
    }
}

pub fn find_visible_windows_by_process_name(process_name: &str) -> Vec<HWND> {
    let mut context = EnumProcessWindowsContext {
        target_process_name: process_name.to_lowercase(),
        own_pid: std::process::id(),
        windows: Vec::new(),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_process_windows_proc),
            LPARAM(&mut context as *mut EnumProcessWindowsContext as isize),
        );
    }

    context.windows
}

struct EnumProcessWindowsContext {
    target_process_name: String,
    own_pid: u32,
    windows: Vec<HWND>,
}

unsafe extern "system" fn enum_process_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = unsafe { &mut *(lparam.0 as *mut EnumProcessWindowsContext) };

    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return BOOL(1);
    }

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 || pid == context.own_pid {
        return BOOL(1);
    }

    if let Some(name) = get_process_name_by_pid(pid)
        && name == context.target_process_name
    {
        context.windows.push(hwnd);
    }

    BOOL(1)
}

fn get_process_name_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;

        // SAFETY: OpenProcess 返回的句柄有效，缓冲区在栈上分配且大小正确。
        // QueryFullProcessImageNameW 会写入缓冲区并更新 size。
        let result = QueryFullProcessImageNameW(
            handle,
            windows::Win32::System::Threading::PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_err() || size == 0 {
            return None;
        }

        let path = String::from_utf16_lossy(&buf[..size as usize]);
        Some(
            Path::new(&path)
                .file_name()?
                .to_string_lossy()
                .to_lowercase()
                .to_string(),
        )
    }
}
