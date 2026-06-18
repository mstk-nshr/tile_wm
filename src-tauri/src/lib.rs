mod app_bar;
mod commands;
mod config;
mod desktop;
mod hotkey;
pub mod tiling;
mod win_event;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub config: Mutex<config::Config>,
    pub current_desktop: Mutex<i32>,
    pub tiling_modes: Mutex<HashMap<i32, tiling::TilingMode>>,
    pub tiling_cycles: Mutex<HashMap<i32, i32>>,
    pub float_window_pos: Mutex<(i32, i32)>,
    pub desktop_thumbnails: Mutex<HashMap<i32, desktop::ThumbnailData>>,
    /// Set to true while the menu is intentionally shown.
    pub menu_shown: Mutex<bool>,
    /// Incremented each time show_menu_window is called.
    /// A monitor thread compares this against its birth generation
    /// to detect stale threads and exit.
    pub menu_generation: Mutex<u64>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = config::load_config();
    let float_pos = (config.window_x, config.window_y);

    // 起動時に実際の仮想デスクトップ番号を取得
    let initial_desktop = desktop::get_current_desktop_number().unwrap_or(1);
    let initial_desktop_for_thread = initial_desktop;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(config.clone()),
            current_desktop: Mutex::new(initial_desktop),
            tiling_modes: Mutex::new(HashMap::new()),
            tiling_cycles: Mutex::new(HashMap::new()),
            float_window_pos: Mutex::new(float_pos),
            desktop_thumbnails: Mutex::new(HashMap::new()),
            menu_shown: Mutex::new(false),
            menu_generation: Mutex::new(0),
        })
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();
            let desktop_count = desktop::get_desktop_count().unwrap_or(4);
            app_bar::register_app_bar(
                &window,
                config.bar_height,
                desktop_count,
                config.window_x,
                config.window_y,
            )?;

            // Start desktop listener thread
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                desktop::listen_desktop_switch(app_handle, initial_desktop_for_thread);
            });

            // Start global hotkey hook thread (Ctrl+Alt+Win+F12/F11)
            let app_handle_hotkey = app.handle().clone();
            std::thread::spawn(move || {
                hotkey::install_hotkey_hook(app_handle_hotkey);
            });

            // Start window creation/destruction monitor
            // Automatically re-tiles when windows appear/disappear in tiling mode
            let app_handle_we = app.handle().clone();
            win_event::listen_window_events(app_handle_we);

            // Start foreground window maximize monitor
            // Lowers tile_wm behind maximized windows and restores topmost on restore
            let app_handle_max = app.handle().clone();
            std::thread::spawn(move || {
                win_event::listen_maximize_events(app_handle_max);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
            commands::get_desktop_apps,
            commands::focus_window,
            commands::close_window,
            commands::move_window_to_desktop,
            commands::get_desktops,
            commands::get_current_desktop,
            commands::switch_desktop,
            commands::get_tiling_mode,
            commands::set_tiling_mode,
            commands::cycle_tiling_layout,
            commands::apply_tiling,
            commands::get_window_list,
            commands::debug_window_list,
            commands::debug_print_windows,
            commands::toggle_devtools_size,
            commands::set_float_pos,
            commands::open_config_file,
            commands::open_help_url,
            commands::show_menu_window,
            commands::hide_menu_window,
            commands::quit_app,
            commands::set_window_size,
            commands::toggle_flip_main,
            commands::capture_current_desktop,
            commands::get_desktop_thumbnail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
