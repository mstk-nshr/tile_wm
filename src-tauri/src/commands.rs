use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config;
use crate::desktop;
use crate::tiling;
use crate::AppState;

const MENU_WIDTH: i32 = 200;
const MENU_HEIGHT: i32 = 200;
const MENU_OFFSET_X: i32 = 20; // メインウィンドウ右端からのオフセット
const MENU_OFFSET_Y: i32 = 4; // メインウィンドウ下端からのオフセット

#[derive(Serialize, Deserialize)]
pub struct ConfigResponse {
    pub bar_height: i32,
    pub split_ratio_x: i32,
    pub split_ratio_y: i32,
    pub exclude_titles: Vec<String>,
    pub exclude_processes: Vec<String>,
    pub float_x: f64,
    pub float_y: f64,
    pub float_width: f64,
    pub float_bg_rgba: [u8; 4],
    pub flip_main: bool,
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> ConfigResponse {
    let config = state.config.lock().unwrap();
    ConfigResponse {
        bar_height: config.bar_height,
        split_ratio_x: config.split_ratio_x,
        split_ratio_y: config.split_ratio_y,
        exclude_titles: config.exclude_titles.clone(),
        exclude_processes: config.exclude_processes.clone(),
        float_x: config.float_x,
        float_y: config.float_y,
        float_width: config.float_width,
        float_bg_rgba: config.float_bg_rgba,
        flip_main: config.flip_main,
    }
}

#[tauri::command]
pub fn update_config(state: State<AppState>, app: tauri::AppHandle, new_config: ConfigResponse) {
    let old_bar_height;
    {
        let mut config = state.config.lock().unwrap();
        old_bar_height = config.bar_height;
        config.bar_height = new_config.bar_height;
        config.split_ratio_x = new_config.split_ratio_x;
        config.split_ratio_y = new_config.split_ratio_y;
        config.exclude_titles = new_config.exclude_titles;
        config.exclude_processes = new_config.exclude_processes;
        config.float_x = new_config.float_x;
        config.float_y = new_config.float_y;
        config.float_width = new_config.float_width;
        config.float_bg_rgba = new_config.float_bg_rgba;
        config.flip_main = new_config.flip_main;
        config::save_config(&config);
    }

    // Resize main window when bar_height changes
    if new_config.bar_height != old_bar_height {
        if let Some(window) = app.get_webview_window("main") {
            let desktop_count = crate::desktop::get_desktop_count().unwrap_or(4);
            let width = crate::app_bar::compute_width(desktop_count);
            let scale_factor = window.scale_factor().unwrap_or(1.0);
            let physical_width = (width as f64 * scale_factor) as i32;
            let physical_height = (new_config.bar_height as f64 * scale_factor) as i32;
            unsafe {
                use windows::Win32::Foundation::{HWND, RECT};
                use windows::Win32::UI::WindowsAndMessaging::*;
                let hwnd = match window.hwnd() {
                    Ok(h) => HWND(h.0),
                    Err(_) => return,
                };

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

                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let x = (screen_w - adjusted_width) / 2;
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    x,
                    0,
                    adjusted_width,
                    adjusted_height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }
}

#[tauri::command]
pub fn get_desktops() -> Vec<i32> {
    // レジストリから実際のデスクトップ数を取得し、失敗時はデフォルトの4を使用
    let count = crate::desktop::get_desktop_count().unwrap_or(4);
    (1..=count).collect()
}

#[tauri::command]
pub fn get_current_desktop(state: State<AppState>) -> i32 {
    *state.current_desktop.lock().unwrap()
}

#[tauri::command]
pub fn switch_desktop(number: i32, state: State<AppState>) -> bool {
    let current = *state.current_desktop.lock().unwrap();
    let diff = (number - current).abs();
    if diff == 0 {
        return true;
    }

    let desktops = match winvd::get_desktops() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[tile_wm] switch_desktop: failed to get desktops: {:?}", e);
            return false;
        }
    };

    let target_index = (number - 1) as usize;
    if target_index < desktops.len() {
        if let Err(e) = winvd::switch_desktop(desktops[target_index]) {
            eprintln!("[tile_wm] switch_desktop: failed to switch: {:?}", e);
            return false;
        }
    } else {
        eprintln!("[tile_wm] switch_desktop: index {} out of bounds", target_index);
        return false;
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
    let new_mode: tiling::TilingMode =
        serde_json::from_str(&mode).unwrap_or(tiling::TilingMode::Free);
    *state.tiling_mode.lock().unwrap() = new_mode;
    // Reset cycle when mode changes
    *state.tiling_cycle.lock().unwrap() = 0;
}

/// Cycle the window-to-tile assignment so the main region gets a different window.
/// The cycle wraps around based on how many windows are being tiled.
#[tauri::command]
pub fn cycle_tiling_layout(state: State<AppState>) {
    let mut cycle = state.tiling_cycle.lock().unwrap();
    *cycle += 1;
}

#[tauri::command]
pub fn apply_tiling(state: State<AppState>) -> bool {
    let mode = *state.tiling_mode.lock().unwrap();
    let config = state.config.lock().unwrap();

    // Get visible windows on current desktop
    let windows = desktop::get_visible_windows();

    // Filter excluded and cloaked windows
    let mut filtered: Vec<_> = windows
        .iter()
        .filter(|w| {
            !w.is_cloaked
                && !config
                    .exclude_processes
                    .iter()
                    .any(|p| w.process_name.contains(p))
                && !config.exclude_titles.iter().any(|t| w.title.contains(t))
        })
        .collect();

    // Sort by HWND for a stable order across cycles (EnumWindows order is not guaranteed)
    filtered.sort_by_key(|w| w.hwnd);

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
        flip_main: config.flip_main,
    };

    let tiles = tiling::calculate_tiles(mode, config_tiling, filtered.len());

    // Apply tile positions to windows with cycle offset so the "main" region
    // rotates among windows each time the user re-clicks the mode button.
    let cycle = *state.tiling_cycle.lock().unwrap();
    let n = tiles.len().min(filtered.len());

    for i in 0..n {
        let src_idx = (i + cycle as usize) % filtered.len();
        let window_info = filtered[src_idx];
        let tile = &tiles[i];

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

    windows
        .into_iter()
        .filter(|w| {
            !w.is_cloaked
                && !config
                    .exclude_processes
                    .iter()
                    .any(|p| w.process_name.contains(p))
                && !config.exclude_titles.iter().any(|t| w.title.contains(t))
        })
        .collect()
}

/// Debug: print ALL visible windows (including cloaked) to the Rust stdout
/// (visible in the terminal running `cargo tauri dev` or the built executable).
#[tauri::command]
pub fn debug_print_windows() {
    let windows = desktop::get_visible_windows();
    let non_cloaked: Vec<_> = windows.iter().filter(|w| !w.is_cloaked).collect();

    println!("");
    println!("═══════════════════════════════════════════════════════");
    println!("[tile_wm DEBUG] EnumWindows detected {} windows", windows.len());
    println!("  Non-cloaked (tiling targets): {}", non_cloaked.len());
    println!("");
    println!("  {:<3} {:<50} {:<20} {:<9} {:<10} {}", "#", "Title", "Process", "Cloaked", "Minimized", "HWND");
    println!("  {:-<3} {:-<50} {:-<20} {:-<9} {:-<10} {:-<10}", "", "", "", "", "", "");
    for (i, w) in windows.iter().enumerate() {
        let cloaked = if w.is_cloaked { "CLOAKED" } else { "—" };
        let minimized = if w.is_minimized { "minimized" } else { "—" };
        let title = truncate_title(&w.title, 48);
        println!("  {:<3} {:<50} {:<20} {:<9} {:<10} {}", i, title, w.process_name, cloaked, minimized, w.hwnd);
    }
    println!("═══════════════════════════════════════════════════════");
    println!("");
}

/// Truncate a string at a UTF-8 char boundary so slicing never panics.
fn truncate_title(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Walk backward from max_bytes until we hit a char boundary
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Debug: return ALL visible windows including cloaked ones, with cloaked status.
/// Useful to understand what windows EnumWindows detects on the current desktop.
#[tauri::command]
pub fn debug_window_list() -> Vec<desktop::WindowInfo> {
    desktop::get_visible_windows()
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

    let main_hwnd = main.hwnd().map_err(|e| format!("hwnd: {}", e))?;

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
        tauri::WebviewWindowBuilder::new(&app, "menu", tauri::WebviewUrl::App("menu.html".into()))
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

    // Remove Windows 11 accent-color border
    crate::app_bar::remove_window_border(&menu);

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

/// Resize the main window to exactly fit the rendered taskbar content width.
/// Called from JS after DOM is fully laid out.
#[tauri::command]
pub fn set_window_size(app: tauri::AppHandle, width: i32, height: i32) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width as f64 * scale_factor) as i32;
    let physical_height = (height as f64 * scale_factor) as i32;

    unsafe {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::UI::WindowsAndMessaging::*;
        let hwnd = match window.hwnd() {
            Ok(h) => HWND(h.0),
            Err(e) => return Err(format!("hwnd: {}", e)),
        };

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

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - adjusted_width) / 2;
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            0,
            adjusted_width,
            adjusted_height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    Ok(())
}

/// Toggle DevTools-friendly window size.
/// Normal = taskbar height; Debug = 800x600 so DevTools console is visible.
#[tauri::command]
pub fn toggle_devtools_size(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let cfg = state.config.lock().unwrap();
    let normal_h = cfg.bar_height;
    drop(cfg);

    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let hwnd = match window.hwnd() {
        Ok(h) => HWND(h.0),
        Err(e) => return Err(format!("hwnd: {}", e)),
    };

    unsafe {
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let current_client_h = rect.bottom - rect.top;

        let normal_phys = (normal_h as f64 * scale_factor) as i32;
        let threshold = normal_phys * 2;

        if current_client_h > threshold {
            // Restore to normal (taskbar height only)
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                (screen_w - (rect.right - rect.left)) / 2,
                0,
                rect.right - rect.left,
                normal_phys,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        } else {
            // Expand to 800x600 for DevTools
            let debug_w = (800_f64 * scale_factor) as i32;
            let debug_h = (600_f64 * scale_factor) as i32;
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let x = (screen_w - debug_w) / 2;
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                0,
                debug_w,
                debug_h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
    Ok(())
}

/// Toggle the flip_main flag — reverses the main window position (left↔right)
/// in the tiling layout. The setting is persisted to config and all tiled windows
/// are immediately updated.
#[tauri::command]
pub fn toggle_flip_main(state: State<AppState>) -> bool {
    let flipped;
    {
        let mut config = state.config.lock().unwrap();
        config.flip_main = !config.flip_main;
        flipped = config.flip_main;
        config::save_config(&config);
    }
    // Reset cycle so flip swaps main↔sub directly instead of cycling
    *state.tiling_cycle.lock().unwrap() = 0;
    flipped
}
