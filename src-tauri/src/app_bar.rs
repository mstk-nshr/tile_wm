use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Calculate taskbar window width based on desktop count.
/// Formula: 440 + 30 * desktop_count
/// - desktop_section: 20px padding-left + N*28px buttons + (N-1)*2px gaps
/// - tiling_section: ~342px (4 fixed buttons)
/// - menu_section: 28px button + 20px padding-right
/// - separators: 2 × 13px, taskbar gaps: 4 × 4px
pub fn compute_width(desktop_count: i32) -> i32 {
    440 + 30 * desktop_count
}

/// Disable the DWM non-client area rendering (border) for an undecorated window.
/// Uses DWMWA_NCRENDERING_POLICY = DWMNCRP_DISABLED to fully remove the border.
pub fn remove_window_border(window: &tauri::WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        let hwnd = HWND(hwnd.0);
        unsafe {
            // DWMNCRP_DISABLED = 1: Disable DWM non-client rendering entirely
            let policy: u32 = 1; // DWMNCRP_DISABLED
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_NCRENDERING_POLICY,
                &policy as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

pub fn register_app_bar(window: &tauri::WebviewWindow, height: i32, desktop_count: i32) -> Result<(), Box<dyn std::error::Error>> {
    let width = compute_width(desktop_count);

    unsafe {
        let hwnd = get_window_handle(window)?;

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - width) / 2;

        // Position window at center-top with computed width
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            0,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );

        Ok(())
    }
}

// pub fn unregister_app_bar(_window: &tauri::WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
//     Ok(())
// }

fn get_window_handle(window: &tauri::WebviewWindow) -> Result<HWND, Box<dyn std::error::Error>> {
    match window.hwnd() {
        Ok(hwnd) => Ok(HWND(hwnd.0)),
        Err(_) => Err("Failed to get window handle".into()),
    }
}
