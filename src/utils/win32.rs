use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetForegroundWindow, GetWindowLongPtrW,
    GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, PostMessageW, SW_RESTORE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WM_CLOSE,
};
use windows::core::PCWSTR;

// SAFETY: FindWindowW 使用由 title 转换出的空结尾宽字符串。
// 返回的 HWND 可能无效或为空，返回前会通过 is_invalid() 检查。
pub fn find_window(title: &str) -> Option<HWND> {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd = FindWindowW(None, PCWSTR::from_raw(wide.as_ptr()));
        if let Ok(hwnd) = hwnd
            && !hwnd.is_invalid()
        {
            Some(hwnd)
        } else {
            None
        }
    }
}

// SAFETY: PostMessageW 只向 find_window 返回的有效 HWND 发送 WM_CLOSE。
// 该操作不通过 HWND 访问内存，只投递窗口消息。
pub fn close_window(title: &str) {
    if let Some(hwnd) = find_window(title) {
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, None, None);
        }
    }
}

// SAFETY: restore_and_activate_window 只对已验证的 HWND 执行窗口 UI 操作。
pub fn bring_window_to_front(title: &str) {
    if let Some(hwnd) = find_window(title) {
        restore_and_activate_window(hwnd);
    }
}

// SAFETY: ShowWindow、SetWindowPos 和前台激活 API 只对调用方传入的 HWND
// 执行窗口 UI 操作。跨完整性级别或前台激活限制导致失败时会静默忽略。
pub fn restore_and_activate_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW;
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags);
        let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
        activate_window_with_attached_input(hwnd);
    }
}

unsafe fn activate_window_with_attached_input(hwnd: HWND) {
    let current_thread = unsafe { GetCurrentThreadId() };
    let target_thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
    let foreground_hwnd = unsafe { GetForegroundWindow() };
    let foreground_thread = if foreground_hwnd.0.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(foreground_hwnd, None) }
    };

    let attached_foreground = foreground_thread != 0
        && foreground_thread != current_thread
        && unsafe { AttachThreadInput(current_thread, foreground_thread, true).as_bool() };
    let attached_target = target_thread != 0
        && target_thread != current_thread
        && unsafe { AttachThreadInput(current_thread, target_thread, true).as_bool() };

    unsafe {
        let _ = BringWindowToTop(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }

    if attached_target {
        let _ = unsafe { AttachThreadInput(current_thread, target_thread, false) };
    }
    if attached_foreground {
        let _ = unsafe { AttachThreadInput(current_thread, foreground_thread, false) };
    }
}

// SAFETY: GetWindowLongPtrW 读取有效 HWND 的扩展窗口样式，
// SetWindowLongPtrW 写回更新后的样式。样式位运算不访问额外内存。
pub fn modify_window_ex_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (current | add_flags) & !remove_flags;
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
    }
}

// SAFETY: GetWindowLongPtrW 读取有效 HWND 的窗口样式，
// SetWindowLongPtrW 写回更新后的样式。样式位运算不访问额外内存。
pub fn modify_window_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let new_style = (current | add_flags) & !remove_flags;
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
    }
}

// SAFETY: SetWindowPos 对调用方传入的有效 HWND 调整位置和大小。
// SWP_NOACTIVATE 避免抢焦点，HWND_TOPMOST 保证窗口位于其他窗口上方。
pub fn set_window_topmost(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    }
}
