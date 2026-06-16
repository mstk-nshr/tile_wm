import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ─── Menu item handlers ────────────────────────────────────────────────────
const menuEditConfig = document.getElementById("menu-edit-config");
const menuHelp = document.getElementById("menu-help");
const menuClose = document.getElementById("menu-close");
const menuQuit = document.getElementById("menu-quit");

async function closeMenu() {
  try {
    await invoke("hide_menu_window");
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
  try {
    await invoke("open_help_url");
  } catch (e) {
    console.error("open_help_url failed:", e);
  }
});

menuClose.addEventListener("click", async () => {
  await closeMenu();
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
  } else {
    window.focus();
  }
});

// Close menu on Escape key
const handleEscape = async (e) => {
  if (e.key === "Escape") {
    await closeMenu();
  }
};
window.addEventListener("keydown", handleEscape);
document.addEventListener("keydown", handleEscape);

// Try to focus on load
window.focus();
