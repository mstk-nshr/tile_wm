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
    btn.addEventListener("click", async (e) => {
      const mode = btn.dataset.mode;
      // CTRL+click on 2/3/4Win toggles flip_main
      if (e.ctrlKey && mode !== "free") {
        try {
          const flipped = await invoke("toggle_flip_main");
          if (config) config.flip_main = flipped;
          // Re-apply tiling with new flip state
          await invoke("apply_tiling");
          updateTilingIcons();
        } catch (e2) {
          console.error("toggle_flip_main failed:", e2);
        }
        return;
      }
      if (mode === currentMode) {
        // Same mode clicked again — cycle the layout
        try {
          await invoke("cycle_tiling_layout");
          await invoke("apply_tiling");
        } catch (e) {
          console.error("Cycle layout failed:", e);
        }
      } else {
        await setTilingMode(mode);
      }
    });
  });
  floatTilingBtns.forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const mode = btn.dataset.mode;
      // CTRL+click on 2/3/4Win toggles flip_main
      if (e.ctrlKey && mode !== "free") {
        try {
          const flipped = await invoke("toggle_flip_main");
          if (config) config.flip_main = flipped;
          await invoke("apply_tiling");
          updateTilingIcons();
        } catch (e2) {
          console.error("toggle_flip_main failed:", e2);
        }
        return;
      }
      if (mode === currentMode) {
        try {
          await invoke("cycle_tiling_layout");
          await invoke("apply_tiling");
        } catch (e) {
          console.error("Cycle layout failed:", e);
        }
      } else {
        await setTilingMode(mode);
      }
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
    if (config && config.bar_height) {
      taskbar.style.height = `${config.bar_height}px`;
      // Adjust button & icon size relative to bar_height (1px margin each side)
      const btnSize = config.bar_height - 2;
      const iconSize = Math.round(btnSize * 0.64);
      taskbar.style.setProperty('--bar-height', `${config.bar_height}px`);
      taskbar.style.setProperty('--btn-size', `${btnSize}px`);
      taskbar.style.setProperty('--icon-size', `${iconSize}px`);
    }
    updateTilingIcons();
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

function tilingIconName(mode) {
  const suffix = config?.flip_main ? "-R.png" : ".png";
  switch (mode) {
    case "2windows": return "/icons/2Win" + suffix;
    case "3windows": return "/icons/3Win" + suffix;
    case "4windows": return "/icons/4Win" + suffix;
    default: return "/icons/free.png";
  }
}

function updateTilingIcons() {
  const modeToId = { "2windows": "tiling-2w", "3windows": "tiling-3w", "4windows": "tiling-4w" };
  for (const [mode, id] of Object.entries(modeToId)) {
    const btn = document.getElementById(id);
    if (btn) {
      const img = btn.querySelector(".icon img");
      if (img) img.src = tilingIconName(mode);
    }
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
    const current = await invoke("get_current_desktop");
    updateDesktopUI(current);
  } catch (e) {
    console.error("Failed to load current desktop state:", e);
    updateDesktopUI(1);
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
  // F12: print window diagnostic list to Rust stdout
  // (visible in the terminal running this app)
  if (e.key === "F12") {
    e.preventDefault();
    try {
      await invoke("debug_print_windows");
    } catch (err) {
      console.error("debug_print_windows failed:", err);
    }
  }
  // Ctrl+D: dump detected windows to console (requires DevTools open)
  if (e.ctrlKey && e.key === "d") {
    e.preventDefault();
    await debugDumpWindows();
  }
});

/// Debug helper: print ALL detected windows (including cloaked) to the browser console.
/// Press Ctrl+D (with DevTools open) to see the list.
async function debugDumpWindows() {
  try {
    const allWindows = await invoke("debug_window_list");
    console.log("═══════════════════════════════════════════");
    console.log("ALL windows detected by EnumWindows:", allWindows.length);
    console.table(
      allWindows.map((w, i) => ({
        "#": i,
        hwnd: w.hwnd,
        title: w.title?.substring(0, 60),
        process: w.process_name,
        cloaked: w.is_cloaked,
        minimized: w.is_minimized,
      }))
    );
    const tiledWindows = allWindows.filter((w) => !w.is_cloaked);
    console.log("Windows used for tiling (non-cloaked):", tiledWindows.length);
    console.table(
      tiledWindows.map((w, i) => ({
        "#": i,
        title: w.title?.substring(0, 60),
        process: w.process_name,
      }))
    );
    console.log("═══════════════════════════════════════════");
  } catch (e) {
    console.error("debugDumpWindows failed:", e);
  }
}
