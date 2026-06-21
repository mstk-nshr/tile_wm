use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::Manager;
use windows::Win32::Foundation::{BOOL, FALSE, HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::commands;
use crate::desktop;
use crate::tiling;

/// Global AppHandle for the win-event thread.
static APP_HANDLE: OnceLock<Mutex<tauri::AppHandle>> = OnceLock::new();

/// Entry point: install the window-creation monitor.
/// Spawns a background thread that polls EnumWindows and automatically
/// applies tiling when windows are added or removed while a tiling mode
/// (2/3/4Win) is active.
pub fn listen_window_events(app_handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(Mutex::new(app_handle.clone()));

    std::thread::spawn(move || {
        run_poll_loop();
    });
}

fn run_poll_loop() {
    let mut known_hwnds: HashSet<isize> = HashSet::new();
    let mut last_change: Option<Instant> = None;
    let poll_interval = Duration::from_millis(500);
    // Wait this long after the last change before applying tiling
    // (debounce: avoids reacting to transient intermediate window states)
    let settle_time = Duration::from_millis(600);

    // Initial snapshot — don't tile existing windows on startup
    if let Some(current) = snapshot_hwnds() {
        known_hwnds = current;
    }

    loop {
        std::thread::sleep(poll_interval);

        let guard = match APP_HANDLE.get() {
            Some(g) => g.lock().unwrap(),
            None => continue,
        };
        let app_handle = guard.clone();
        drop(guard);

        // Check if we are in a tiling mode
        let state = app_handle.state::<crate::AppState>();
        let desktop = *state.current_desktop.lock().unwrap();
        let mode = state
            .tiling_modes
            .lock()
            .unwrap()
            .get(&desktop)
            .copied()
            .unwrap_or(tiling::TilingMode::Free);

        if mode == tiling::TilingMode::Free {
            // In Free mode, just keep the snapshot up to date
            if let Some(current) = snapshot_hwnds() {
                known_hwnds = current;
            }
            last_change = None;
            continue;
        }

        // Take current snapshot
        let current = match snapshot_hwnds() {
            Some(c) => c,
            None => continue,
        };

        // Detect changes
        let added: Vec<_> = current.difference(&known_hwnds).copied().collect();
        let removed: Vec<_> = known_hwnds.difference(&current).copied().collect();

        if !added.is_empty() || !removed.is_empty() {
            if !added.is_empty() {
                println!(
                    "[tile_wm] win_event: {} new window(s) detected, scheduling tile",
                    added.len()
                );
            }
            if !removed.is_empty() {
                println!(
                    "[tile_wm] win_event: {} window(s) removed, scheduling tile",
                    removed.len()
                );
            }
            known_hwnds = current;
            last_change = Some(Instant::now());
        }

        // Apply tiling after the settle period
        if let Some(changed_at) = last_change {
            if changed_at.elapsed() >= settle_time {
                println!("[tile_wm] win_event: applying tiling (mode={:?})", mode);
                commands::apply_tiling_internal(&state);
                last_change = None;
            }
        }
    }
}

/// Take a snapshot of all HWNDs that are eligible for tiling
/// (same filtering as apply_tiling_internal).
fn snapshot_hwnds() -> Option<HashSet<isize>> {
    // We can't easily read the exclusion config from here without AppState,
    // but we can approximate by getting all visible windows that aren't cloaked.
    // The actual filtering happens in apply_tiling_internal anyway.
    let windows = desktop::get_visible_windows();
    let hwnds: HashSet<isize> = windows
        .into_iter()
        .filter(|w| !w.is_cloaked)
        .map(|w| w.hwnd)
        .collect();
    Some(hwnds)
}

/// Monitor foreground window maximize/restore events and adjust tile_wm's
/// z-order accordingly. When any window is maximized, tile_wm removes its
/// topmost style and places itself below the maximized window so it stays
/// hidden. When the maximized window is restored, tile_wm regains topmost.
pub fn listen_maximize_events(app_handle: tauri::AppHandle) {
    // Get tile_wm main window HWND
    let tile_hwnd = match app_handle.get_webview_window("main") {
        Some(w) => match w.hwnd() {
            Ok(h) => HWND(h.0),
            Err(_) => {
                eprintln!("[tile_wm] listen_maximize_events: failed to get main window HWND");
                return;
            }
        },
        None => {
            eprintln!("[tile_wm] listen_maximize_events: main window not found");
            return;
        }
    };

    // Get menu window HWND (optional, for exclusion)
    let menu_hwnd = app_handle
        .get_webview_window("menu")
        .and_then(|w| w.hwnd().ok())
        .map(|h| HWND(h.0));

    let mut was_lowered = false;
    // Track the HWND that caused the lowering so we can detect if it's still maximized/fullscreen
    let mut maximized_hwnd: Option<HWND> = None;
    let poll_interval = Duration::from_millis(300);

    println!("[tile_wm] listen_maximize_events: started");

    loop {
        std::thread::sleep(poll_interval);

        unsafe {
            let foreground = GetForegroundWindow();

            // Skip if invalid (no foreground window / desktop focused)
            if foreground.is_invalid() {
                // Desktop has focus: check if any window is still maximized/fullscreen
                if was_lowered && !any_maximized_window_exists(tile_hwnd, menu_hwnd) {
                    restore_topmost(tile_hwnd, &mut was_lowered, &mut maximized_hwnd);
                }
                continue;
            }

            // Skip tile_wm own windows (main window or menu)
            if foreground == tile_hwnd || menu_hwnd == Some(foreground) {
                continue;
            }

            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..WINDOWPLACEMENT::default()
            };

            let is_max_or_fs = if GetWindowPlacement(foreground, &mut placement).is_ok() {
                placement.showCmd == SW_SHOWMAXIMIZED.0 as u32 || is_window_fullscreen(foreground)
            } else {
                is_window_fullscreen(foreground)
            };

            if is_max_or_fs {
                // Foreground window is maximized or fullscreen — lower tile_wm
                if !was_lowered || maximized_hwnd != Some(foreground) {
                    was_lowered = true;
                    maximized_hwnd = Some(foreground);
                    lower_tile_wm(tile_hwnd, foreground);
                }
            } else if was_lowered {
                // Foreground window is no longer maximized/fullscreen, but another
                // window might still be — scan all windows before restoring
                if !any_maximized_window_exists(tile_hwnd, menu_hwnd) {
                    restore_topmost(tile_hwnd, &mut was_lowered, &mut maximized_hwnd);
                }
            }
        }
    }
}

/// Helper function to check if a window is fullscreen (occupies the entire monitor).
unsafe fn is_window_fullscreen(hwnd: HWND) -> bool {
    let mut rect = windows::Win32::Foundation::RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return false;
    }

    let monitor = windows::Win32::Graphics::Gdi::MonitorFromWindow(
        hwnd,
        windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
    );
    if monitor.is_invalid() {
        return false;
    }

    let mut monitor_info = windows::Win32::Graphics::Gdi::MONITORINFO {
        cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
        ..Default::default()
    };

    if windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
        rect.left <= monitor_info.rcMonitor.left
            && rect.top <= monitor_info.rcMonitor.top
            && rect.right >= monitor_info.rcMonitor.right
            && rect.bottom >= monitor_info.rcMonitor.bottom
    } else {
        false
    }
}

/// Remove tile_wm's topmost style and place it below the maximized window
/// in the z-order so the maximized window covers it.
unsafe fn lower_tile_wm(tile_hwnd: HWND, maximized: HWND) {
    // Step 1: Remove WS_EX_TOPMOST style so the window behaves as a normal window
    let _ = SetWindowPos(
        tile_hwnd,
        HWND_NOTOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    // Step 2: Place tile_wm directly below the maximized window in z-order
    let _ = SetWindowPos(
        tile_hwnd,
        maximized,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    println!("[tile_wm] lowered behind maximized window");
}

/// Restore tile_wm to topmost position.
unsafe fn restore_topmost(
    tile_hwnd: HWND,
    was_lowered: &mut bool,
    maximized_hwnd: &mut Option<HWND>,
) {
    let _ = SetWindowPos(
        tile_hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    *was_lowered = false;
    *maximized_hwnd = None;
    println!("[tile_wm] restored to topmost");
}

/// Scan all visible top-level windows to check if any (other than tile_wm's
/// own windows) are currently maximized or fullscreen.
unsafe fn any_maximized_window_exists(tile_hwnd: HWND, menu_hwnd: Option<HWND>) -> bool {
    struct Ctx {
        found: bool,
        tile_hwnd: HWND,
        menu_hwnd: Option<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);

        // Skip tile_wm's own windows
        if hwnd == ctx.tile_hwnd {
            return TRUE;
        }
        if let Some(menu) = ctx.menu_hwnd {
            if hwnd == menu {
                return TRUE;
            }
        }

        // Only consider visible windows
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }

        // Skip tool windows and cloaked windows (same filtering as get_visible_windows)
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if ex_style & (WS_EX_TOOLWINDOW.0 as i32) != 0 {
            return TRUE;
        }

        let mut is_cloaked: BOOL = BOOL::default();
        let _ = windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(14), // DWMWA_CLOAKED
            &mut is_cloaked as *mut BOOL as *mut std::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
        if is_cloaked.as_bool() {
            return TRUE;
        }

        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..WINDOWPLACEMENT::default()
        };

        let is_max_or_fs = if GetWindowPlacement(hwnd, &mut placement).is_ok() {
            placement.showCmd == SW_SHOWMAXIMIZED.0 as u32 || is_window_fullscreen(hwnd)
        } else {
            is_window_fullscreen(hwnd)
        };

        if is_max_or_fs {
            ctx.found = true;
            return FALSE; // Stop enumerating
        }

        TRUE
    }

    let mut ctx = Ctx {
        found: false,
        tile_hwnd,
        menu_hwnd,
    };

    let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut Ctx as isize));
    ctx.found
}
