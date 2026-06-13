use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::Registry::*;
use windows::Win32::Graphics::Dwm::DwmGetWindowAttribute;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::*;
use windows::core::{w, PWSTR, PCWSTR};
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
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let b = u8::from_str_radix(&s[i..i + 2], 16).ok()?;
        bytes.push(b);
    }
    Some(bytes)
}

/// Poll the Windows registry for the current virtual desktop number.
/// When it changes (user pressed Ctrl+Win+Arrow or switched via Task View),
/// update AppState and emit a "desktop-changed" event to the frontend.
pub fn listen_desktop_switch(app_handle: tauri::AppHandle, initial_desktop: i32) {
    let mut last_desktop: i32 = initial_desktop;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        let current = match get_current_desktop_number() {
            Some(n) => n,
            None => continue,
        };

        if current != last_desktop {
            last_desktop = current;

            let state = app_handle.state::<super::AppState>();
            if let Ok(mut desktop) = state.current_desktop.lock() {
                *desktop = current;
            }
            let _ = app_handle.emit("desktop-changed", current);
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
    pub is_cloaked: bool,
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

    // Skip cloaked windows (Windows 11 virtual desktop ghosts,
    // background app windows that are visually hidden but still return IsWindowVisible=TRUE)
    let mut is_cloaked: BOOL = BOOL::default();
    let _ = DwmGetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(14), // DWMWA_CLOAKED
        &mut is_cloaked as *mut BOOL as *mut std::ffi::c_void,
        std::mem::size_of::<BOOL>() as u32,
    );
    if is_cloaked.as_bool() {
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
        is_cloaked: is_cloaked.as_bool(),
    });

    TRUE
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DesktopApp {
    pub hwnd: isize,
    pub process_name: String,
    pub icon_base64: Option<String>,
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        match chunk.len() {
            3 => {
                let b = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
                result.push(CHARSET[((b >> 18) & 63) as usize] as char);
                result.push(CHARSET[((b >> 12) & 63) as usize] as char);
                result.push(CHARSET[((b >> 6) & 63) as usize] as char);
                result.push(CHARSET[(b & 63) as usize] as char);
            }
            2 => {
                let b = ((chunk[0] as u32) << 8) | (chunk[1] as u32);
                result.push(CHARSET[((b >> 10) & 63) as usize] as char);
                result.push(CHARSET[((b >> 4) & 63) as usize] as char);
                result.push(CHARSET[((b << 2) & 63) as usize] as char);
                result.push('=');
            }
            1 => {
                let b = chunk[0] as u32;
                result.push(CHARSET[((b >> 2) & 63) as usize] as char);
                result.push(CHARSET[((b << 4) & 63) as usize] as char);
                result.push('=');
                result.push('=');
            }
            _ => unreachable!(),
        }
    }
    result
}

fn hicon_to_bmp_base64(hicon: HICON) -> Option<String> {
    unsafe {
        let mut icon_info = ICONINFO::default();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            return None;
        }
        
        let mut res = None;
        let hdc = CreateCompatibleDC(None);
        if !hdc.is_invalid() {
            let mut bmp = BITMAP::default();
            if GetObjectW(icon_info.hbmColor, std::mem::size_of::<BITMAP>() as i32, Some(&mut bmp as *mut _ as *mut _)) != 0 {
                let width = bmp.bmWidth;
                let height = bmp.bmHeight;
                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height, // top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut buffer = vec![0u8; (width * height * 4) as usize];
                if GetDIBits(hdc, icon_info.hbmColor, 0, height as u32, Some(buffer.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS) != 0 {
                    let file_header_size = std::mem::size_of::<BITMAPFILEHEADER>();
                    let info_header_size = std::mem::size_of::<BITMAPINFOHEADER>();
                    let total_size = file_header_size + info_header_size + buffer.len();
                    let mut bmp_file = Vec::with_capacity(total_size);
                    let file_header = BITMAPFILEHEADER {
                        bfType: 0x4D42, // 'BM'
                        bfSize: total_size as u32,
                        bfReserved1: 0,
                        bfReserved2: 0,
                        bfOffBits: (file_header_size + info_header_size) as u32,
                    };
                    let file_header_bytes = std::slice::from_raw_parts(&file_header as *const _ as *const u8, file_header_size);
                    bmp_file.extend_from_slice(file_header_bytes);
                    let info_header_bytes = std::slice::from_raw_parts(&bmi.bmiHeader as *const _ as *const u8, info_header_size);
                    bmp_file.extend_from_slice(info_header_bytes);
                    bmp_file.extend_from_slice(&buffer);
                    res = Some(format!("data:image/bmp;base64,{}", base64_encode(&bmp_file)));
                }
            }
            let _ = DeleteDC(hdc);
        }
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        res
    }
}

fn get_process_icon(process_path: &str) -> Option<HICON> {
    let mut shfi = SHFILEINFOW::default();
    let path_u16: Vec<u16> = process_path.encode_utf16().chain(Some(0)).collect();
    let res = unsafe {
        SHGetFileInfoW(
            PCWSTR(path_u16.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi as *mut _ as *mut _),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };
    if res != 0 && !shfi.hIcon.is_invalid() {
        Some(shfi.hIcon)
    } else {
        None
    }
}

fn get_window_icon(hwnd: HWND) -> Option<HICON> {
    unsafe {
        let mut result: usize = 0;
        let res = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(2), // ICON_SMALL2
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            100,
            Some(&mut result),
        );
        if res.0 != 0 && result != 0 {
            return Some(HICON(result as *mut std::ffi::c_void));
        }

        let res = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(0), // ICON_SMALL
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            100,
            Some(&mut result),
        );
        if res.0 != 0 && result != 0 {
            return Some(HICON(result as *mut std::ffi::c_void));
        }

        let hicon = GetClassLongPtrW(hwnd, GCLP_HICONSM);
        if hicon != 0 {
            return Some(HICON(hicon as *mut std::ffi::c_void));
        }

        let hicon = GetClassLongPtrW(hwnd, GCLP_HICON);
        if hicon != 0 {
            return Some(HICON(hicon as *mut std::ffi::c_void));
        }
    }
    None
}

unsafe extern "system" fn enum_all_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<(HWND, String, String, String)>);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }

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
    let mut process_path = String::new();
    let mut process_name = String::new();
    if pid > 0 {
        if let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let _ = QueryFullProcessImageNameW(proc, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size);
            let _ = CloseHandle(proc);
            process_path = String::from_utf16_lossy(&buf[..size as usize]);
            process_name = std::path::Path::new(&process_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    }

    windows.push((hwnd, title, process_name, process_path));
    TRUE
}

pub fn get_all_desktops_apps(
    exclude_processes: &[String],
    exclude_titles: &[String],
) -> std::collections::HashMap<i32, Vec<DesktopApp>> {
    let mut map: std::collections::HashMap<i32, Vec<DesktopApp>> = std::collections::HashMap::new();
    let desktops = match winvd::get_desktops() {
        Ok(d) => d,
        Err(_) => return map,
    };

    let mut windows: Vec<(HWND, String, String, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_all_windows_callback),
            LPARAM(&mut windows as *mut Vec<(HWND, String, String, String)> as isize),
        );
    }

    for (hwnd, title, process_name, process_path) in windows {
        if exclude_processes.iter().any(|p| process_name.contains(p))
            || exclude_titles.iter().any(|t| title.contains(t))
        {
            continue;
        }

        for (idx, &d) in desktops.iter().enumerate() {
            let desktop_num = (idx + 1) as i32;
            if winvd::is_window_on_desktop(d, hwnd).unwrap_or(false) {
                let apps = map.entry(desktop_num).or_default();
                if !apps.iter().any(|app| app.process_name == process_name) {
                    let mut icon_base64 = None;

                    if let Some(hicon) = get_window_icon(hwnd) {
                        icon_base64 = hicon_to_bmp_base64(hicon);
                    }

                    if icon_base64.is_none() && !process_path.is_empty() {
                        if let Some(hicon) = get_process_icon(&process_path) {
                            icon_base64 = hicon_to_bmp_base64(hicon);
                            unsafe { let _ = DestroyIcon(hicon); }
                        }
                    }

                    apps.push(DesktopApp {
                        hwnd: hwnd.0 as isize,
                        process_name: process_name.clone(),
                        icon_base64,
                    });
                }
            }
        }
    }
    map
}