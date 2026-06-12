mod app_bar;
mod desktop;
pub mod tiling;
mod config;
mod commands;
mod hotkey;

use tauri::Manager;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<config::Config>,
    pub current_desktop: Mutex<i32>,
    pub tiling_mode: Mutex<tiling::TilingMode>,
    pub tiling_cycle: Mutex<i32>,
    pub float_window_pos: Mutex<(f64, f64)>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = config::load_config();
    let float_pos = (config.float_x, config.float_y);

    // 起動時に実際の仮想デスクトップ番号を取得
    let initial_desktop = desktop::get_current_desktop_number().unwrap_or(1);
    let initial_desktop_for_thread = initial_desktop;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(config.clone()),
            current_desktop: Mutex::new(initial_desktop),
            tiling_mode: Mutex::new(tiling::TilingMode::Free),
            tiling_cycle: Mutex::new(0),
            float_window_pos: Mutex::new(float_pos),
        })
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();
            let desktop_count = desktop::get_desktop_count().unwrap_or(4);
            app_bar::register_app_bar(&window, config.bar_height, desktop_count)?;

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
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
            commands::show_menu_window,
            commands::hide_menu_window,
            commands::quit_app,
            commands::set_window_size,
            commands::toggle_flip_main,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
