use std::sync::{Mutex, OnceLock};
use tauri::Emitter;
use tauri::Manager;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

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
    if target_vk != VK_F12.0 && target_vk != VK_F11.0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // 修飾キー状態チェック
    let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as i16) < 0;
    let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as i16) < 0;
    let win = (GetAsyncKeyState(VK_LWIN.0 as i32) as i16) < 0
        || (GetAsyncKeyState(VK_RWIN.0 as i32) as i16) < 0;

    if !(ctrl && alt && win) {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
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