use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::*;
use windows::core::PWSTR;
use tauri::{Emitter, Manager};

pub fn listen_desktop_switch(app_handle: tauri::AppHandle) {
    let mut last_fg_hwnd: isize = 0;
    let mut desktop_counter: i32 = 1;
    let mut last_switch_time = std::time::Instant::now();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));

        unsafe {
            let fg = GetForegroundWindow();
            let current_hwnd = fg.0 as isize;

            if current_hwnd != last_fg_hwnd {
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_switch_time);

                if elapsed.as_millis() < 500 && current_hwnd != 0 && last_fg_hwnd != 0 {
                    desktop_counter = (desktop_counter % 10) + 1;
                    last_switch_time = now;

                    let state = app_handle.state::<super::AppState>();
                    if let Ok(mut current) = state.current_desktop.lock() {
                        *current = desktop_counter;
                        let _ = app_handle.emit("desktop-changed", desktop_counter);
                    };
                }
                last_fg_hwnd = current_hwnd;
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub rect: (i32, i32, i32, i32),
    pub is_visible: bool,
    pub is_minimized: bool,
}

pub fn get_visible_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }

    windows
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }

    // Skip tool windows
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if ex_style & (WS_EX_TOOLWINDOW.0 as i32) != 0 {
        return TRUE;
    }

    let mut title_buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut title_buf);
    let title = String::from_utf16_lossy(&title_buf[..len as usize]).trim().to_string();
    if title.is_empty() {
        return TRUE;
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let process_name = if pid > 0 {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if let Ok(proc) = handle {
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let _ = QueryFullProcessImageNameW(proc, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size);
            let _ = CloseHandle(proc);
            let path = String::from_utf16_lossy(&buf[..size as usize]);
            std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);
    let is_minimized = IsIconic(hwnd).as_bool();

    windows.push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        process_name,
        rect: (rect.left, rect.top, rect.right, rect.bottom),
        is_visible: true,
        is_minimized,
    });

    TRUE
}