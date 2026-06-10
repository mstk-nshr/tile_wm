use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::Registry::*;
use windows::core::{w, PWSTR};
use tauri::{Emitter, Manager};

/// Windows レジストリから現在の仮想デスクトップ番号（1-based）を取得する。
/// レジストリキー:
///   HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops
///   - CurrentVirtualDesktop : REG_SZ  (現在のデスクトップ GUID 文字列)
///   - VirtualDesktopIDs     : REG_BINARY (全デスクトップ GUID を連結したバイナリ)
pub fn get_current_desktop_number() -> Option<i32> {
    unsafe {
        let mut key = HKEY::default();
        let path = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops");
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            path,
            0,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        // --- CurrentVirtualDesktop ---
        let mut value_type = REG_VALUE_TYPE::default();
        let mut buf_size: u32 = 0;
        if RegQueryValueExW(
            key,
            w!("CurrentVirtualDesktop"),
            None,
            Some(&mut value_type),
            None,
            Some(&mut buf_size),
        )
        .is_err()
        {
            let _ = RegCloseKey(key);
            return None;
        }

        let mut val_buf = vec![0u8; buf_size as usize];
        if RegQueryValueExW(
            key,
            w!("CurrentVirtualDesktop"),
            None,
            Some(&mut value_type),
            Some(val_buf.as_mut_ptr()),
            Some(&mut buf_size),
        )
        .is_err()
        {
            let _ = RegCloseKey(key);
            return None;
        }

        let cur_guid_bin = if value_type == REG_BINARY {
            if val_buf.len() != 16 {
                let _ = RegCloseKey(key);
                return None;
            }
            val_buf
        } else if value_type == REG_SZ {
            let u16_chars = val_buf
                .chunks_exact(2)
                .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<u16>>();
            let len = u16_chars.iter().position(|&x| x == 0).unwrap_or(u16_chars.len());
            let guid_str = String::from_utf16_lossy(&u16_chars[..len]);
            let bin = parse_guid_string(&guid_str);
            if bin.is_none() {
                let _ = RegCloseKey(key);
                return None;
            }
            bin.unwrap()
        } else {
            let _ = RegCloseKey(key);
            return None;
        };

        // --- VirtualDesktopIDs (REG_BINARY) ---
        let mut bin_size: u32 = 0;
        let mut value_type2 = REG_VALUE_TYPE::default();
        if RegQueryValueExW(
            key,
            w!("VirtualDesktopIDs"),
            None,
            Some(&mut value_type2),
            None,
            Some(&mut bin_size),
        )
        .is_err()
        {
            let _ = RegCloseKey(key);
            return None;
        }

        let mut bin_buf: Vec<u8> = vec![0u8; bin_size as usize];
        let mut value_type3 = REG_VALUE_TYPE::default();
        if RegQueryValueExW(
            key,
            w!("VirtualDesktopIDs"),
            None,
            Some(&mut value_type3),
            Some(bin_buf.as_mut_ptr()),
            Some(&mut bin_size),
        )
        .is_err()
        {
            let _ = RegCloseKey(key);
            return None;
        }
        let _ = RegCloseKey(key);

        // 16 バイトずつ走査してインデックスを探す
        for (i, chunk) in bin_buf.chunks(16).enumerate() {
            if chunk.len() == 16 && chunk == cur_guid_bin.as_slice() {
                return Some((i + 1) as i32); // 1-based
            }
        }

        None
    }
}

/// Windows レジストリから仮想デスクトップの総数（1-based の最大番号）を取得する。
/// VirtualDesktopIDs のバイナリから 16 バイトチャンク数を数える。
pub fn get_desktop_count() -> Option<i32> {
    unsafe {
        let mut key = HKEY::default();
        let path = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops");
        if RegOpenKeyExW(HKEY_CURRENT_USER, path, 0, KEY_READ, &mut key).is_err() {
            return None;
        }

        let mut bin_size: u32 = 0;
        let mut value_type = REG_VALUE_TYPE::default();
        if RegQueryValueExW(
            key,
            w!("VirtualDesktopIDs"),
            None,
            Some(&mut value_type),
            None,
            Some(&mut bin_size),
        )
        .is_err()
        {
            let _ = RegCloseKey(key);
            return None;
        }

        let _ = RegCloseKey(key);
        // 各デスクトップは 16 バイトの GUID
        Some((bin_size / 16) as i32)
    }
}

/// GUID 文字列 "{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}" を
/// 16 バイトのバイナリ GUID に変換する。
fn parse_guid_string(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let s = s.strip_prefix('{').unwrap_or(s);
    let s = s.strip_suffix('}').unwrap_or(s);
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return None;
    }

    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;
    let data4 = hex_str_to_bytes(parts[3])?;
    let data5 = hex_str_to_bytes(parts[4])?;

    let mut bytes = Vec::with_capacity(16);
    // Data1 – little-endian
    bytes.extend_from_slice(&data1.to_le_bytes());
    // Data2 – little-endian
    bytes.extend_from_slice(&data2.to_le_bytes());
    // Data3 – little-endian
    bytes.extend_from_slice(&data3.to_le_bytes());
    // Data4 (2 bytes) – big-endian (in-order)
    bytes.extend_from_slice(&data4);
    // Data5 (6 bytes) – big-endian (in-order)
    bytes.extend_from_slice(&data5);

    Some(bytes)
}

fn hex_str_to_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let b = u8::from_str_radix(&s[i..i + 2], 16).ok()?;
        bytes.push(b);
    }
    Some(bytes)
}

pub fn listen_desktop_switch(app_handle: tauri::AppHandle, initial_desktop: i32) {
    let mut last_fg_hwnd: isize = 0;
    let mut desktop_counter: i32 = initial_desktop;
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