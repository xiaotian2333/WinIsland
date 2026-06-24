use std::path::Path;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
};

pub fn get_foreground_process_name() -> Option<String> {
    // SAFETY: 这些 Win32 调用只查询当前前台窗口、窗口类名和进程 ID。
    // 类名缓冲区在栈上分配且长度正确，返回的 HWND 和 PID 不会跨函数保存。
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

    // SAFETY: EnumWindows 同步调用回调函数。context 在调用期间位于栈上且保持有效，
    // LPARAM 只在回调内还原为同一类型的可变指针，枚举结束后才读取结果。
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
    // SAFETY: lparam 来自 find_visible_windows_by_process_name 传入的 context 指针，
    // EnumWindows 在该栈变量有效期间同步调用本回调。窗口查询 API 只读取 hwnd 状态。
    let (context, visible, pid) = unsafe {
        let context = &mut *(lparam.0 as *mut EnumProcessWindowsContext);
        let visible = IsWindowVisible(hwnd).as_bool();
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (context, visible, pid)
    };

    if !visible {
        return BOOL(1);
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
