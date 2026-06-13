use windows::Win32::Foundation::{HWND, RECT};
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

pub fn register_app_bar(
    window: &tauri::WebviewWindow,
    height: i32,
    desktop_count: i32,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = compute_width(desktop_count);
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width as f64 * scale_factor) as i32;
    let physical_height = (height as f64 * scale_factor) as i32;

    unsafe {
        let hwnd = get_window_handle(window)?;

        let mut rect_window = RECT::default();
        let mut rect_client = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect_window);
        let _ = GetClientRect(hwnd, &mut rect_client);

        let border_width =
            (rect_window.right - rect_window.left) - (rect_client.right - rect_client.left);
        let border_height =
            (rect_window.bottom - rect_window.top) - (rect_client.bottom - rect_client.top);

        let adjusted_width = physical_width + border_width;
        let adjusted_height = physical_height + border_height;

        // Position window at (x, y) with computed width
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            adjusted_width,
            adjusted_height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );

        Ok(())
    }
}

fn get_window_handle(window: &tauri::WebviewWindow) -> Result<HWND, Box<dyn std::error::Error>> {
    match window.hwnd() {
        Ok(hwnd) => Ok(HWND(hwnd.0)),
        Err(_) => Err("Failed to get window handle".into()),
    }
}
