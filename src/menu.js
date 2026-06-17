import { invoke } from "@tauri-apps/api/core";

// ─── Menu item handlers ────────────────────────────────────────────────────
const menuEditConfig = document.getElementById("menu-edit-config");
const menuHelp = document.getElementById("menu-help");
const menuClose = document.getElementById("menu-close");
const menuQuit = document.getElementById("menu-quit");

const sliderSplitX = document.getElementById("slider-split-x");
const sliderSplitY = document.getElementById("slider-split-y");
const valSplitX = document.getElementById("val-split-x");
const valSplitY = document.getElementById("val-split-y");

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

async function updateSplitRatio(axis, value) {
  try {
    const currentConfig = await invoke("get_config");
    if (axis === "x") {
      currentConfig.split_ratio_x = parseInt(value);
    } else {
      currentConfig.split_ratio_y = parseInt(value);
    }
    await invoke("update_config", { newConfig: currentConfig });
  } catch (e) {
    console.error(`update_config failed for ${axis}:`, e);
  }
}

sliderSplitX.addEventListener("input", (e) => {
  const val = e.target.value;
  valSplitX.textContent = val;
  updateSplitRatio("x", val);
});

sliderSplitY.addEventListener("input", (e) => {
  const val = e.target.value;
  valSplitY.textContent = val;
  updateSplitRatio("y", val);
});

// Initialize sliders with current config
async function initSliders() {
  try {
    const config = await invoke("get_config");
    sliderSplitX.value = config.split_ratio_x;
    valSplitX.textContent = config.split_ratio_x;
    sliderSplitY.value = config.split_ratio_y;
    valSplitY.textContent = config.split_ratio_y;
  } catch (e) {
    console.error("initSliders failed:", e);
  }
}
initSliders();

// ─── Keyboard shortcuts ────────────────────────────────────────────────────

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
