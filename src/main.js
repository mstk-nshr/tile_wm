import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ─── State ─────────────────────────────────────────────────────────────────
let currentDesktop = 1;
let currentMode = "free";
let config = null;

// ─── DOM refs ─────────────────────────────────────────────────────────────
const taskbar = document.getElementById("taskbar");
const desktopSection = document.getElementById("desktop-section");
const tilingBtns = document.querySelectorAll(".tiling-btn");
const menuBtn = document.getElementById("menu-btn");
const floatWindow = document.getElementById("float-window");
const floatHeader = document.getElementById("float-header");
const floatDesktopBtns = document.getElementById("float-desktop-btns");
const floatTilingBtns = document.querySelectorAll(".float-tiling-btn");
let desktopBtns = [];
let floatDskBtns = [];

// ─── Init ─────────────────────────────────────────────────────────────────
async function init() {
  let desktopList = [];
  try {
    desktopList = await invoke("get_desktops");
  } catch (e) {
    console.error("Failed to get desktops:", e);
    desktopList = [1, 2, 3, 4]; // fallback
  }

  // Dynamically create desktop buttons
  createDesktopButtons(desktopList);

  try {
    await loadConfig();
  } catch (e) {
    console.error("Config load failed (non-fatal):", e);
  }
  try {
    await loadDesktopState();
  } catch (e) {
    console.error(e);
  }
  try {
    await loadTilingState();
  } catch (e) {
    console.error(e);
  }

  // Desktop switching
  desktopBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const num = parseInt(btn.dataset.desktop);
      switchDesktop(num);
    });
  });
  floatDskBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const num = parseInt(btn.dataset.desktop);
      switchDesktop(num);
    });
  });

  // Tiling mode switching
  tilingBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const mode = btn.dataset.mode;
      setTilingMode(mode);
    });
  });
  floatTilingBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const mode = btn.dataset.mode;
      setTilingMode(mode);
    });
  });

  // Menu — opens a separate popup window via Rust
  menuBtn.addEventListener("click", async (e) => {
    e.stopPropagation();
    try {
      await invoke("show_menu_window");
    } catch (err) {
      console.error("show_menu_window failed:", err);
    }
  });

  // Float window drag
  setupFloatWindowDrag();

  // Listen for desktop changes from system (Ctrl+Win+Arrow)
  try {
    await listen("desktop-changed", (event) => {
      updateDesktopUI(event.payload);
    });
  } catch (e) {
    console.error("Event listen failed:", e);
  }

  // Measure rendered content width and resize window to fit exactly
  requestAnimationFrame(() => {
    // Double rAF ensures layout is complete before measuring
    requestAnimationFrame(() => fitWindowToContent());
  });
}

// ─── Config ───────────────────────────────────────────────────────────────
async function loadConfig() {
  try {
    config = await invoke("get_config");
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

// ─── Desktop ──────────────────────────────────────────────────────────────
function createDesktopButtons(desktopList) {
  desktopSection.innerHTML = "";
  floatDesktopBtns.innerHTML = "";

  desktopBtns = [];
  floatDskBtns = [];

  desktopList.forEach((num) => {
    const btn = document.createElement("div");
    btn.className = "desktop-btn";
    btn.dataset.desktop = num;
    btn.textContent = num;
    desktopSection.appendChild(btn);
    desktopBtns.push(btn);

    const fbtn = document.createElement("div");
    fbtn.className = "float-desktop-btn";
    fbtn.dataset.desktop = num;
    fbtn.textContent = num;
    floatDesktopBtns.appendChild(fbtn);
    floatDskBtns.push(fbtn);
  });
}

async function loadDesktopState() {
  try {
    // TODO: get current desktop from backend
    updateDesktopUI(1);
  } catch (e) {
    console.error(e);
  }
}

async function switchDesktop(num) {
  try {
    await invoke("switch_desktop", { number: num });
    currentDesktop = num;
    updateDesktopUI(num);
  } catch (e) {
    console.error("Failed to switch desktop:", e);
  }
}

function updateDesktopUI(num) {
  currentDesktop = num;
  desktopBtns.forEach((btn) => {
    btn.classList.toggle("active", parseInt(btn.dataset.desktop) === num);
  });
  floatDskBtns.forEach((btn) => {
    btn.classList.toggle("active", parseInt(btn.dataset.desktop) === num);
  });
}

// ─── Tiling ────────────────────────────────────────────────────────────────
async function loadTilingState() {
  try {
    const modeJson = await invoke("get_tiling_mode");
    const mode = JSON.parse(modeJson);
    updateTilingUI(mode);
  } catch (e) {
    console.error(e);
  }
}

async function setTilingMode(mode) {
  try {
    await invoke("set_tiling_mode", { mode: JSON.stringify(mode) });
    currentMode = mode;
    updateTilingUI(mode);
    // Apply tiling automatically
    try {
      await invoke("apply_tiling");
    } catch (e2) {
      console.error("Tiling apply failed:", e2);
    }
  } catch (e) {
    console.error("Failed to set tiling mode:", e);
  }
}

function updateTilingUI(mode) {
  currentMode = mode;
  tilingBtns.forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.mode === mode);
  });
  floatTilingBtns.forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.mode === mode);
  });
}

// ─── Float Window Drag ────────────────────────────────────────────────────
function setupFloatWindowDrag() {
  let isDragging = false;
  let startX, startY, startLeft, startTop;

  floatHeader.addEventListener("mousedown", (e) => {
    isDragging = true;
    const rect = floatWindow.getBoundingClientRect();
    startX = e.clientX;
    startY = e.clientY;
    startLeft = rect.left;
    startTop = rect.top;
    floatHeader.style.cursor = "grabbing";
  });

  document.addEventListener("mousemove", (e) => {
    if (!isDragging) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    floatWindow.style.left = startLeft + dx + "px";
    floatWindow.style.top = startTop + dy + "px";
  });

  document.addEventListener("mouseup", () => {
    if (isDragging) {
      isDragging = false;
      floatHeader.style.cursor = "grab";
      const rect = floatWindow.getBoundingClientRect();
      invoke("set_float_pos", { x: rect.left, y: rect.top });
    }
  });
}

// ─── Window Size ──────────────────────────────────────────────────────────

/// Measure the actual rendered width of the taskbar and resize the window
/// to fit exactly, eliminating any gaps on the sides.
async function fitWindowToContent() {
  const contentWidth = taskbar.scrollWidth;
  const height = config?.bar_height ?? 40;
  try {
    await invoke("set_window_size", { width: contentWidth, height: height });
  } catch (e) {
    console.error("set_window_size failed:", e);
  }
}

// ─── Start ────────────────────────────────────────────────────────────────
document.addEventListener("DOMContentLoaded", init);

// Close menu window when Escape is pressed on the main window
window.addEventListener("keydown", async (e) => {
  if (e.key === "Escape") {
    try {
      await invoke("hide_menu_window");
    } catch (err) {
      console.error("Failed to hide menu window via ESC:", err);
    }
  }
});
