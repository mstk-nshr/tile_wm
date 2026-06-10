import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ─── Menu item handlers ────────────────────────────────────────────────────
const menuEditConfig = document.getElementById("menu-edit-config");
const menuHelp = document.getElementById("menu-help");
const menuQuit = document.getElementById("menu-quit");

async function closeMenu() {
  try {
    const win = getCurrentWindow();
    await win.hide();
  } catch (e) {
    console.error("Hide failed:", e);
  }
}

menuEditConfig.addEventListener("click", async () => {
  await closeMenu();
  try {
    await invoke("open_config_file");
  } catch (e) {
    console.error("open_config_file failed:", e);
  }
});

menuHelp.addEventListener("click", async () => {
  await closeMenu();
  alert(
    "tile_wm v0.1.0\n\n" +
      "Top taskbar with virtual desktop switching\n" +
      "and window tiling for Windows 11.\n\n" +
      "Tiling modes:\n" +
      "  Free - No tiling\n" +
      "  2Win - Split left/right\n" +
      "  3Win - Main + 2 stack\n" +
      "  4Win - 2x2 grid\n\n" +
      "Drag the menu button to reposition."
  );
});

menuQuit.addEventListener("click", async () => {
  await closeMenu();
  try {
    await invoke("quit_app");
  } catch (e) {
    console.error("quit_app failed:", e);
  }
});

// ─── Auto-close on focus loss ──────────────────────────────────────────────
const win = getCurrentWindow();
win.onFocusChanged(({ payload: focused }) => {
  if (!focused) {
    win.hide().catch((e) => console.error("Hide on blur failed:", e));
  }
});
