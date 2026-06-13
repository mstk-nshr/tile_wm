use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::Manager;

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
