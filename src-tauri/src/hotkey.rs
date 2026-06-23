use std::sync::{Mutex, OnceLock};
use tauri::Emitter;
use tauri::Manager;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;

// ─── グローバル状態 ─────────────────────────────────────────────────────────

static APP_HANDLE: OnceLock<Mutex<tauri::AppHandle>> = OnceLock::new();

/// 自分の SendInput によるキーイベントを区別する
static INJECTING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

// ─── KBDLLHOOKSTRUCT ────────────────────────────────────────────────────────

/// WH_KEYBOARD_LL の lParam が指す構造体（windows crate に定義無し）
#[repr(C)]
#[derive(Clone, Copy)]
struct KbdLlHookStruct {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    dw_extra_info: usize,
}

const HC_ACTION: i32 = 0;

// ─── Public Entry Point ─────────────────────────────────────────────────────

/// WH_KEYBOARD_LL 低レベルキーボードフックをインストールする。
/// このフックは **キー入力をブロックできる** ため、他アプリに横取りされない。
///
/// ショートカット:
///   Ctrl+Alt+Win+F11 → フォアグラウンドウィンドウを左のデスクトップに移動
///   Ctrl+Alt+Win+F12 → フォアグラウンドウィンドウを右のデスクトップに移動
///
/// 移動後、移動先のデスクトップに切り替え、移動したウィンドウにフォーカスを戻す。
pub fn install_hotkey_hook(app_handle: tauri::AppHandle) {
    // AppHandle をグローバルに保存（フックコールバックからアクセスするため）
    let _ = APP_HANDLE.set(Mutex::new(app_handle));

    unsafe {
        let hook_proc: HOOKPROC = Some(hook_callback);

        let hmod = HINSTANCE::default(); // WH_KEYBOARD_LL は NULL でOK（hook proc は同一プロセス内）
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, hmod, 0) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[tile_wm] SetWindowsHookExW(WH_KEYBOARD_LL) FAILED: {:?}", e);
                return;
            }
        };
        println!("[tile_wm] WH_KEYBOARD_LL installed (Ctrl+Win+Alt+F11/F12)");

        // メッセージループ（フックが機能するために必要）
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }

        // アプリ終了時
        let _ = UnhookWindowsHookEx(hook);
        println!("[tile_wm] WH_KEYBOARD_LL unhooked");
    }
}

// ─── Snapping State & Logic ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SnapState {
    Normal,
    Maximized,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

static NORMAL_BOUNDS: OnceLock<Mutex<std::collections::HashMap<isize, RECT>>> = OnceLock::new();

unsafe fn get_monitor_info(hwnd: HWND) -> Option<MONITORINFO> {
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if monitor.is_invalid() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut info).as_bool() {
        Some(info)
    } else {
        None
    }
}

fn fire_snap(vk_code: u16, hwnd: HWND) {
    // 1. Get current snap state and spacing config
    let (top_sp, bottom_sp, left_sp, right_sp, inner_sp, ratio_x, ratio_y, flip_main) = {
        if let Some(handle_mutex) = APP_HANDLE.get() {
            if let Ok(handle) = handle_mutex.lock() {
                let state = handle.state::<crate::AppState>();
                let config = state.config.lock().unwrap();
                (
                    config.top_spacing,
                    config.bottom_spacing,
                    config.left_spacing,
                    config.right_spacing,
                    config.inner_spacing,
                    config.split_ratio_x as f64 / 100.0,
                    config.split_ratio_y as f64 / 100.0,
                    config.flip_main,
                )
            } else {
                (40, 10, 10, 10, 10, 0.5, 0.5, false)
            }
        } else {
            (40, 10, 10, 10, 10, 0.5, 0.5, false)
        }
    };

    let hwnd_raw = hwnd.0 as isize;
    let mut current_state = SnapState::Normal;

    if let Some(handle_mutex) = APP_HANDLE.get() {
        if let Ok(handle) = handle_mutex.lock() {
            let state = handle.state::<crate::AppState>();
            let mut snap_states = state.snap_states.lock().unwrap();
            current_state = *snap_states.entry(hwnd_raw).or_insert(SnapState::Normal);
        }
    }

    // 2. Query monitor info
    let monitor_info = unsafe {
        match get_monitor_info(hwnd) {
            Some(info) => info,
            None => return,
        }
    };
    let rc_work = monitor_info.rcWork;
    let monitor_x = monitor_info.rcMonitor.left;
    let monitor_w = monitor_info.rcMonitor.right - monitor_info.rcMonitor.left;

    // Calculate dimensions
    let work_x = rc_work.left + left_sp;
    let work_y = rc_work.top + top_sp;
    let work_w = (rc_work.right - rc_work.left) - left_sp - right_sp;
    let work_h = (rc_work.bottom - rc_work.top) - top_sp - bottom_sp;

    let half_w = ((work_w - inner_sp) as f64 * ratio_x) as i32;
    let other_w = work_w - half_w - inner_sp;
    let half_h = ((work_h - inner_sp) as f64 * ratio_y) as i32;
    let other_h = work_h - half_h - inner_sp;

    // Define helper to save normal bounds if currently Normal
    let save_normal_bounds = || {
        let mut rect = RECT::default();
        unsafe {
            let _ = GetWindowRect(hwnd, &mut rect);
        }
        let bounds_map = NORMAL_BOUNDS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
        if let Ok(mut map) = bounds_map.lock() {
            map.insert(hwnd_raw, rect);
        }
    };

    let get_normal_bounds = || {
        let bounds_map = NORMAL_BOUNDS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
        if let Ok(map) = bounds_map.lock() {
            map.get(&hwnd_raw).cloned()
        } else {
            None
        }
    };

    // 3. Determine next state and target bounds
    let mut next_state = current_state;
    let mut should_minimize = false;

    // When flip_main is active, WIN+LEFT should snap to the visually left side
    // (which is actually the "right" region in unflipped coordinates), so swap the key.
    let effective_vk = if flip_main {
        if vk_code == VK_LEFT.0 { VK_RIGHT.0 }
        else if vk_code == VK_RIGHT.0 { VK_LEFT.0 }
        else { vk_code }
    } else {
        vk_code
    };

    match effective_vk {
        c if c == VK_LEFT.0 => {
            match current_state {
                SnapState::Right => {
                    next_state = SnapState::Normal;
                }
                SnapState::TopRight => {
                    next_state = SnapState::TopLeft;
                }
                SnapState::BottomRight => {
                    next_state = SnapState::BottomLeft;
                }
                SnapState::Left | SnapState::TopLeft | SnapState::BottomLeft => {
                    // Stay or do nothing
                }
                SnapState::Normal | SnapState::Maximized => {
                    if current_state == SnapState::Normal {
                        save_normal_bounds();
                    }
                    next_state = SnapState::Left;
                }
            }
        }
        c if c == VK_RIGHT.0 => {
            match current_state {
                SnapState::Left => {
                    next_state = SnapState::Normal;
                }
                SnapState::TopLeft => {
                    next_state = SnapState::TopRight;
                }
                SnapState::BottomLeft => {
                    next_state = SnapState::BottomRight;
                }
                SnapState::Right | SnapState::TopRight | SnapState::BottomRight => {
                    // Stay or do nothing
                }
                SnapState::Normal | SnapState::Maximized => {
                    if current_state == SnapState::Normal {
                        save_normal_bounds();
                    }
                    next_state = SnapState::Right;
                }
            }
        }
        c if c == VK_UP.0 => {
            match current_state {
                SnapState::Left => {
                    next_state = SnapState::TopLeft;
                }
                SnapState::Right => {
                    next_state = SnapState::TopRight;
                }
                SnapState::BottomLeft => {
                    next_state = SnapState::Normal;
                }
                SnapState::BottomRight => {
                    next_state = SnapState::Normal;
                }
                SnapState::Normal => {
                    save_normal_bounds();
                    next_state = SnapState::Maximized;
                }
                SnapState::Maximized => {
                    // Already maximized
                }
                SnapState::TopLeft | SnapState::TopRight => {
                    // Already at top
                }
            }
        }
        c if c == VK_DOWN.0 => {
            match current_state {
                SnapState::Maximized => {
                    next_state = SnapState::Normal;
                }
                SnapState::Left => {
                    next_state = SnapState::BottomLeft;
                }
                SnapState::Right => {
                    next_state = SnapState::BottomRight;
                }
                SnapState::TopLeft => {
                    next_state = SnapState::Normal;
                }
                SnapState::TopRight => {
                    next_state = SnapState::Normal;
                }
                SnapState::Normal => {
                    should_minimize = true;
                }
                SnapState::BottomLeft | SnapState::BottomRight => {
                    // Already at bottom
                }
            }
        }
        _ => return,
    }

    // 4. Calculate target position based on next_state
    let target_rect = match next_state {
        SnapState::Normal => {
            if let Some(rect) = get_normal_bounds() {
                Some((
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                ))
            } else {
                // Sane default: Center half size of work area
                Some((
                    work_x + work_w / 4,
                    work_y + work_h / 4,
                    work_w / 2,
                    work_h / 2,
                ))
            }
        }
        SnapState::Maximized => {
            Some((work_x, work_y, work_w, work_h))
        }
        SnapState::Left => {
            Some((work_x, work_y, half_w, work_h))
        }
        SnapState::Right => {
            Some((work_x + half_w + inner_sp, work_y, other_w, work_h))
        }
        SnapState::TopLeft => {
            Some((work_x, work_y, half_w, half_h))
        }
        SnapState::BottomLeft => {
            Some((work_x, work_y + half_h + inner_sp, half_w, other_h))
        }
        SnapState::TopRight => {
            Some((work_x + half_w + inner_sp, work_y, other_w, half_h))
        }
        SnapState::BottomRight => {
            Some((
                work_x + half_w + inner_sp,
                work_y + half_h + inner_sp,
                other_w,
                other_h,
            ))
        }
    };

    // Update snap state in map
    if let Some(handle_mutex) = APP_HANDLE.get() {
        if let Ok(handle) = handle_mutex.lock() {
            let state = handle.state::<crate::AppState>();
            let mut snap_states = state.snap_states.lock().unwrap();
            snap_states.insert(hwnd_raw, next_state);
        }
    }

    // Apply snap position or minimize
    unsafe {
        if should_minimize {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
        } else if let Some((mut x, y, w, h)) = target_rect {
            if flip_main {
                let right = x + w;
                x = monitor_x + (monitor_w - (right - monitor_x));
            }
            // Restore window if minimized or OS-maximized before positioning
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if IsZoomed(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

// ─── Hook Callback ──────────────────────────────────────────────────────────

unsafe extern "system" fn hook_callback(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < HC_ACTION {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // 自分で注入したキーは無視
    if INJECTING.load(std::sync::atomic::Ordering::SeqCst) {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // WM_KEYDOWN のみ処理
    if wparam.0 != WM_KEYDOWN as usize {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KbdLlHookStruct);

    // 目的のキーか確認
    let target_vk = kb.vk_code as u16;
    let is_arrow = target_vk == VK_LEFT.0 || target_vk == VK_RIGHT.0 || target_vk == VK_UP.0 || target_vk == VK_DOWN.0;
    let is_f_key = target_vk == VK_F12.0 || target_vk == VK_F11.0;

    if !is_arrow && !is_f_key {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // 修飾キー状態チェック
    let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as i16) < 0;
    let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as i16) < 0;
    let win = (GetAsyncKeyState(VK_LWIN.0 as i32) as i16) < 0
        || (GetAsyncKeyState(VK_RWIN.0 as i32) as i16) < 0;
    let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as i16) < 0;

    if is_arrow {
        if !(win && !ctrl && !alt && !shift) {
            return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
        }
    } else {
        if !(ctrl && alt && win) {
            return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
        }
    }

    if is_arrow {
        let target_hwnd = GetForegroundWindow();
        let target_hwnd_raw = target_hwnd.0 as isize;
        std::thread::spawn(move || {
            let hwnd = HWND(target_hwnd_raw as *mut std::ffi::c_void);
            fire_snap(target_vk, hwnd);
        });
        return LRESULT(1); // ブロック！
    }

    let direction: i32 = if target_vk == VK_F11.0 { -1 } else { 1 };

    // 移動対象のウィンドウ HWND を記録（フック callback はフォアグラウンドスレッド内）
    let target_hwnd = GetForegroundWindow();
    // HWND は *mut c_void なので Send ではない。isize に変換してスレッドに渡す。
    let target_hwnd_raw = target_hwnd.0 as isize;

    println!(
        "[tile_wm] HOOK BLOCKED: Ctrl+Win+Alt+{} ({}) hwnd=0x{:X}",
        if direction < 0 { "F11" } else { "F12" },
        if direction < 0 { "LEFT" } else { "RIGHT" },
        target_hwnd_raw
    );

    // キーを **ブロック**（非ゼロ戻り値で他アプリに渡らない）
    // ウィンドウ移動を実行
    std::thread::spawn(move || {
        let hwnd = HWND(target_hwnd_raw as *mut std::ffi::c_void);
        fire_move(direction, hwnd);
    });

    LRESULT(1) // ブロック！
}

// ─── ウィンドウ移動 + デスクトップ切替 + フォーカス復帰 ─────────────────────

/// フォアグラウンドウィンドウを隣のデスクトップに移動し、
/// 移動先のデスクトップに切り替えた後、そのウィンドウにフォーカスを戻す。
fn fire_move(direction: i32, target_hwnd: HWND) {
    let desktops = match winvd::get_desktops() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[tile_wm] Failed to get desktops: {:?}", e);
            return;
        }
    };
    
    let current_desktop = match winvd::get_current_desktop() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[tile_wm] Failed to get current desktop: {:?}", e);
            return;
        }
    };
    
    // Find index of current desktop
    let mut current_index = None;
    for (i, d) in desktops.iter().enumerate() {
        if d == &current_desktop {
            current_index = Some(i);
            break;
        }
    }
    
    let current_index = match current_index {
        Some(i) => i,
        None => {
            eprintln!("[tile_wm] Current desktop not found in list");
            return;
        }
    };
    
    let n = desktops.len() as i32;
    let target_index = (current_index as i32 + direction).rem_euclid(n) as usize;
    let target_desktop = &desktops[target_index];
    
    // ── Step 1: ウィンドウを目的のデスクトップへ移動 ──
    if let Err(e) = winvd::move_window_to_desktop(*target_desktop, &target_hwnd) {
        eprintln!("[tile_wm] Failed to move window: {:?}", e);
    }
    
    // ウィンドウ移動が完了するのを少し待つ
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // ── Step 2: デスクトップを切り替え ──
    if let Err(e) = winvd::switch_desktop(*target_desktop) {
        eprintln!("[tile_wm] Failed to switch desktop: {:?}", e);
    }
    
    // デスクトップ切替アニメーションを待つ
    std::thread::sleep(std::time::Duration::from_millis(300));
    
    // ── Step 3: 移動したウィンドウにフォーカスを再設定 ──
    // Skip if the menu is currently shown to avoid stealing focus and
    // triggering the menu's focus-loss close logic.
    let menu_shown = APP_HANDLE.get().and_then(|hm| {
        hm.lock().ok().and_then(|handle| {
            handle.try_state::<crate::AppState>()
                .map(|s| *s.menu_shown.lock().unwrap())
        })
    }).unwrap_or(false);

    if !menu_shown {
        unsafe {
            use windows::Win32::System::Threading::GetCurrentProcessId;
            let _ = AllowSetForegroundWindow(GetCurrentProcessId());

            let _ = SetForegroundWindow(target_hwnd);
            let _ = SetFocus(target_hwnd);

            println!(
                "[tile_wm] Re-focused window hwnd={:?}",
                target_hwnd
            );
        }
    }

    // ── 内部カウンター更新 ──
    if let Some(handle_mutex) = APP_HANDLE.get() {
        if let Ok(handle) = handle_mutex.lock() {
            if let Ok(mut c) = handle.state::<crate::AppState>().current_desktop.lock() {
                // UI は 1-based なので +1
                let new_val = target_index as i32 + 1;
                *c = new_val;
                println!("[tile_wm] desktop-changed: {}", new_val);
                let _ = handle.emit("desktop-changed", new_val);
            }
        }
    }
}