use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

const COMPACT_WIDTH: i32 = 570;

pub fn register_app_bar(window: &tauri::WebviewWindow, height: i32) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let hwnd = get_window_handle(window)?;

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - COMPACT_WIDTH) / 2;

        // Position window at center-top with compact width
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            0,
            COMPACT_WIDTH,
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
