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
    pub snap_states: Mutex<HashMap<isize, hotkey::SnapState>>,
    /// Set to true while the menu is intentionally shown.
    pub menu_shown: Mutex<bool>,
    /// Incremented each time show_menu_window is called.
    /// A monitor thread compares this against its birth generation
    /// to detect stale threads and exit.
    pub menu_generation: Mutex<u64>,
}

use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, WAIT_OBJECT_0, WAIT_ABANDONED};
use windows::Win32::System::Threading::{CreateMutexW, CreateEventW, SetEvent, ResetEvent, WaitForSingleObject, INFINITE};
use windows::core::w;

struct SendHandle(isize);
unsafe impl Send for SendHandle {}

unsafe fn handle_single_instance() -> Result<(), windows::core::Error> {
    // Create or open a named event to signal termination
    let terminate_event = CreateEventW(None, true, false, w!("Local\\tile_wm_terminate_event"))?;

    // Create a named mutex to detect running instances
    let mutex = CreateMutexW(None, false, w!("Local\\tile_wm_singleton_mutex"))?;

    if GetLastError() == ERROR_ALREADY_EXISTS {
        println!("[tile_wm] Another instance of tile_wm is already running. Signaling it to exit...");
        SetEvent(terminate_event)?;

        // Wait up to 5 seconds for the other instance to exit and release the mutex
        let wait_res = WaitForSingleObject(mutex, 5000);
        if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
            println!("[tile_wm] Successfully acquired singleton mutex. Old instance exited.");
        } else {
            println!("[tile_wm] Warning: Wait timed out or failed ({:?}). Continuing anyway.", wait_res);
        }

        // Reset the event so we don't immediately terminate ourselves when we listen
        ResetEvent(terminate_event)?;
    }

    let terminate_event_send = SendHandle(terminate_event.0 as isize);
    // Spawn a listener thread to exit when the terminate event is signaled by a new instance
    std::thread::spawn(move || {
        unsafe {
            let handle = windows::Win32::Foundation::HANDLE(terminate_event_send.0 as *mut std::ffi::c_void);
            let wait_res = WaitForSingleObject(handle, INFINITE);
            if wait_res == WAIT_OBJECT_0 {
                println!("[tile_wm] Another instance started. Exiting...");
                std::process::exit(0);
            }
        }
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    unsafe {
        if let Err(e) = handle_single_instance() {
            eprintln!("[tile_wm] Error initializing single instance: {:?}", e);
        }
    }

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
            snap_states: Mutex::new(HashMap::new()),
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
