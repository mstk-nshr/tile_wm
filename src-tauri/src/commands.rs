use tauri::{Manager, State};
use serde::{Deserialize, Serialize};
use windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Foundation::{HWND, RECT};

use crate::AppState;
use crate::config;
use crate::tiling;
use crate::desktop;

const MENU_WIDTH: i32 = 200;
const MENU_HEIGHT: i32 = 170;
const MENU_OFFSET_X: i32 = 20; // メインウィンドウ右端からのオフセット
const MENU_OFFSET_Y: i32 = 4;  // メインウィンドウ下端からのオフセット

#[derive(Serialize, Deserialize)]
pub struct ConfigResponse {
    pub bar_height: i32,
    pub desktop_count: i32,
    pub split_ratio_x: i32,
    pub split_ratio_y: i32,
    pub exclude_titles: Vec<String>,
    pub exclude_processes: Vec<String>,
    pub float_x: f64,
    pub float_y: f64,
    pub float_width: f64,
    pub float_bg_rgba: [u8; 4],
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> ConfigResponse {
    let config = state.config.lock().unwrap();
    ConfigResponse {
        bar_height: config.bar_height,
        desktop_count: config.desktop_count,
        split_ratio_x: config.split_ratio_x,
        split_ratio_y: config.split_ratio_y,
        exclude_titles: config.exclude_titles.clone(),
        exclude_processes: config.exclude_processes.clone(),
        float_x: config.float_x,
        float_y: config.float_y,
        float_width: config.float_width,
        float_bg_rgba: config.float_bg_rgba,
    }
}

#[tauri::command]
pub fn update_config(state: State<AppState>, new_config: ConfigResponse) {
    let mut config = state.config.lock().unwrap();
    config.bar_height = new_config.bar_height;
    config.desktop_count = new_config.desktop_count;
    config.split_ratio_x = new_config.split_ratio_x;
    config.split_ratio_y = new_config.split_ratio_y;
    config.exclude_titles = new_config.exclude_titles;
    config.exclude_processes = new_config.exclude_processes;
    config.float_x = new_config.float_x;
    config.float_y = new_config.float_y;
    config.float_width = new_config.float_width;
    config.float_bg_rgba = new_config.float_bg_rgba;
    config::save_config(&config);
}

#[tauri::command]
pub fn get_desktops(state: State<AppState>) -> Vec<i32> {
    // レジストリから実際のデスクトップ数を取得し、失敗時は config の値を使用
    let count = crate::desktop::get_desktop_count()
        .unwrap_or_else(|| {
            let config = state.config.lock().unwrap();
            config.desktop_count
        });
    (1..=count).collect()
}

#[tauri::command]
pub fn switch_desktop(number: i32, state: State<AppState>) -> bool {
    let current = *state.current_desktop.lock().unwrap();
    let diff = (number - current).abs();
    if diff == 0 {
        return true;
    }

    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let press_mods = [
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL, wScan: 0, dwFlags: KEYBD_EVENT_FLAGS(0), time: 0, dwExtraInfo: 0 } } },
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_LWIN, wScan: 0, dwFlags: KEYBD_EVENT_FLAGS(0), time: 0, dwExtraInfo: 0 } } },
        ];
        let _ = SendInput(&press_mods, std::mem::size_of::<INPUT>() as i32);

        let arrow = if number > current { VK_RIGHT } else { VK_LEFT };

        for _ in 0..diff {
            let strike = [
                INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: arrow, wScan: 0, dwFlags: KEYBD_EVENT_FLAGS(0), time: 0, dwExtraInfo: 0 } } },
                INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: arrow, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } } },
            ];
            let _ = SendInput(&strike, std::mem::size_of::<INPUT>() as i32);
        }

        let release_mods = [
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_LWIN, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } } },
            INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } } },
        ];
        let _ = SendInput(&release_mods, std::mem::size_of::<INPUT>() as i32);
    }

    *state.current_desktop.lock().unwrap() = number;
    true
}

#[tauri::command]
pub fn get_tiling_mode(state: State<AppState>) -> String {
    let mode = state.tiling_mode.lock().unwrap();
    serde_json::to_string(&*mode).unwrap_or_default()
}

#[tauri::command]
pub fn set_tiling_mode(state: State<AppState>, mode: String) {
    let new_mode: tiling::TilingMode = serde_json::from_str(&mode).unwrap_or(tiling::TilingMode::Free);
    *state.tiling_mode.lock().unwrap() = new_mode;
}

#[tauri::command]
pub fn apply_tiling(state: State<AppState>) -> bool {
    let mode = *state.tiling_mode.lock().unwrap();
    let config = state.config.lock().unwrap();
    
    // Get visible windows on current desktop
    let windows = desktop::get_visible_windows();
    
    // Filter excluded windows
    let filtered: Vec<_> = windows.iter().filter(|w| {
        !config.exclude_processes.iter().any(|p| w.process_name.contains(p)) &&
        !config.exclude_titles.iter().any(|t| w.title.contains(t))
    }).collect();
    
    if filtered.is_empty() {
        return false;
    }
    
    // Get primary monitor work area
    let monitor_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let monitor_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    
    let config_tiling = tiling::TilingConfig {
        monitor_x: 0,
        monitor_y: 0,
        monitor_w: monitor_w as i32,
        monitor_h: monitor_h as i32,
        split_ratio_x: config.split_ratio_x,
        split_ratio_y: config.split_ratio_y,
    };

    let tiles = tiling::calculate_tiles(
        mode,
        config_tiling,
        filtered.len(),
    );
    
    // Apply tile positions to windows
    for (i, tile) in tiles.iter().enumerate() {
        if i >= filtered.len() {
            break;
        }

        let window_info = filtered[i];

        unsafe {
            let hwnd: HWND = std::mem::transmute(window_info.hwnd);

            if window_info.is_minimized {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                tile.x,
                tile.y,
                tile.width,
                tile.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
    
    true
}

#[tauri::command]
pub fn get_window_list(state: State<AppState>) -> Vec<desktop::WindowInfo> {
    let config = state.config.lock().unwrap();
    let windows = desktop::get_visible_windows();
    
    windows.into_iter().filter(|w| {
        !config.exclude_processes.iter().any(|p| w.process_name.contains(p)) &&
        !config.exclude_titles.iter().any(|t| w.title.contains(t))
    }).collect()
}

#[tauri::command]
pub fn set_float_pos(state: State<AppState>, x: f64, y: f64) {
    *state.float_window_pos.lock().unwrap() = (x, y);
    let mut config = state.config.lock().unwrap();
    config.float_x = x;
    config.float_y = y;
    config::save_config(&config);
}

#[tauri::command]
pub fn open_config_file() {
    let path = config::config_path();
    if let Err(e) = open::that(&path) {
        log::error!("Failed to open config file: {}", e);
    }
}

// ─── Menu Window Commands ──────────────────────────────────────────────────

/// Show the menu window positioned near the main taskbar's menu button.
/// If the menu window doesn't exist yet, it will be created lazily on first call.
#[tauri::command]
pub async fn show_menu_window(app: tauri::AppHandle) -> Result<(), String> {
    // Get main window rect to position the menu below/right of it
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let main_hwnd = main
        .hwnd()
        .map_err(|e| format!("hwnd: {}", e))?;

    let mut rect = RECT::default();
    let got_rect = unsafe { GetWindowRect(HWND(main_hwnd.0), &mut rect) };
    if got_rect.is_err() {
        return Err("GetWindowRect failed".into());
    }

    // Position menu so its top-right aligns roughly with the right side of the main window
    let menu_x = rect.right - MENU_WIDTH - MENU_OFFSET_X;
    let menu_y = rect.bottom + MENU_OFFSET_Y;

    // Get or create the menu window
    let menu = if let Some(w) = app.get_webview_window("menu") {
        w
    } else {
        tauri::WebviewWindowBuilder::new(
            &app,
            "menu",
            tauri::WebviewUrl::App("menu.html".into()),
        )
        .title("tile_wm menu")
        .inner_size(MENU_WIDTH as f64, MENU_HEIGHT as f64)
        .min_inner_size(MENU_WIDTH as f64, MENU_HEIGHT as f64)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(false)
        .build()
        .map_err(|e| format!("build menu: {}", e))?
    };

    // Move to computed position
    menu.set_position(tauri::PhysicalPosition::new(menu_x, menu_y))
        .map_err(|e| format!("set_position: {}", e))?;

    // Show and focus
    menu.show().map_err(|e| format!("show: {}", e))?;
    menu.set_focus().map_err(|e| format!("set_focus: {}", e))?;

    Ok(())
}

/// Hide the menu window if it's currently shown.
#[tauri::command]
pub fn hide_menu_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(menu) = app.get_webview_window("menu") {
        menu.hide().map_err(|e| format!("hide: {}", e))?;
    }
    Ok(())
}

/// Quit the entire application.
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
