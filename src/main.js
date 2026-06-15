import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ─── State ─────────────────────────────────────────────────────────────────
let currentDesktop = 1;
let currentMode = "free";
let config = null;
let uwpIconSize = 40; // updated by loadConfig()

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
  setupGlobalDragAndDrop();

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
      loadTilingState();
      updateDesktopIcons();
    });
  } catch (e) {
    console.error("Event listen failed:", e);
  }

  updateDesktopIcons();
  setInterval(updateDesktopIcons, 2000);

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
      // UWP icons (square AppxManifest logos) have generous transparent padding,
      // so they can be drawn almost edge-to-edge inside the button.
      // Non-UWP icons already fill the source image, so we keep a small inner margin.
      uwpIconSize = btnSize - 3;              // button border − 3px (inner margin)
      const desktopIconSize = btnSize - 6;     // small inner margin

      // Calculate screen aspect ratio
      const screenWidth = window.screen.width || 1920;
      const screenHeight = window.screen.height || 1080;
      const aspect = screenWidth / screenHeight;

      taskbar.style.setProperty('--bar-height', `${config.bar_height}px`);
      taskbar.style.setProperty('--btn-size', `${btnSize}px`);
      taskbar.style.setProperty('--icon-size', `${iconSize}px`);
      taskbar.style.setProperty('--uwp-icon-size', `${uwpIconSize}px`);
      taskbar.style.setProperty('--desktop-icon-size', `${desktopIconSize}px`);
      taskbar.style.setProperty('--screen-aspect', `${aspect}`);
    }
    if (config && config.window_bg_rgba) {
      const [r, g, b, a] = config.window_bg_rgba;
      taskbar.style.background = `rgba(${r}, ${g}, ${b}, ${a / 255})`;
    }
    if (config && config.button_fg_rgb) {
      taskbar.style.setProperty('--button-fg-rgb', config.button_fg_rgb.join(', '));
    }
    if (config && config.button_bg_rgb) {
      taskbar.style.setProperty('--button-bg-rgb', config.button_bg_rgb.join(', '));
    }
    if (config && config.button_highlight_fg_rgb) {
      taskbar.style.setProperty('--button-highlight-fg-rgb', config.button_highlight_fg_rgb.join(', '));
    }
    if (config && config.button_highlight_bg_rgb) {
      taskbar.style.setProperty('--button-highlight-bg-rgb', config.button_highlight_bg_rgb.join(', '));
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

let globalDnDInitialized = false;
function setupGlobalDragAndDrop() {
  if (globalDnDInitialized) return;
  globalDnDInitialized = true;

  document.addEventListener("dragover", (e) => {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "move";
    }
  });

  document.addEventListener("dragenter", (e) => {
    e.preventDefault();
    document.querySelectorAll(".desktop-item-container, .float-desktop-btn").forEach(el => {
      el.classList.remove("drag-hover");
    });

    const container = e.target.closest(".desktop-item-container");
    if (container) {
      container.classList.add("drag-hover");
    } else {
      const floatBtn = e.target.closest(".float-desktop-btn");
      if (floatBtn) {
        floatBtn.classList.add("drag-hover");
      }
    }
  });

  document.addEventListener("dragleave", (e) => {
    const container = e.target.closest(".desktop-item-container");
    const floatBtn = e.target.closest(".float-desktop-btn");
    if (container && !container.contains(e.relatedTarget)) {
      container.classList.remove("drag-hover");
    }
    if (floatBtn && !floatBtn.contains(e.relatedTarget)) {
      floatBtn.classList.remove("drag-hover");
    }
  });

  document.addEventListener("dragend", () => {
    document.querySelectorAll(".desktop-item-container, .float-desktop-btn").forEach(el => {
      el.classList.remove("drag-hover");
    });
    taskbar.classList.remove("dragging-active");
  });

  document.addEventListener("drop", async (e) => {
    e.preventDefault();
    document.querySelectorAll(".desktop-item-container, .float-desktop-btn").forEach(el => {
      el.classList.remove("drag-hover");
    });
    taskbar.classList.remove("dragging-active");

    const hwndStr = e.dataTransfer.getData("text");
    const hwnd = parseInt(hwndStr, 10);

    const container = e.target.closest(".desktop-item-container");
    const floatBtn = e.target.closest(".float-desktop-btn");

    let targetDesktop = null;
    if (container) {
      targetDesktop = parseInt(container.dataset.desktop, 10);
    } else if (floatBtn) {
      targetDesktop = parseInt(floatBtn.dataset.desktop, 10);
    }

    if (!isNaN(hwnd) && targetDesktop !== null && !isNaN(targetDesktop)) {
      try {
        await invoke("move_window_to_desktop", { hwnd, desktopNumber: targetDesktop });
        await switchDesktop(targetDesktop);
        await invoke("focus_window", { hwnd });
      } catch (err) {
        console.error("Failed to drag-and-drop move window:", err);
      }
    }
  });
}

// ─── Desktop ──────────────────────────────────────────────────────────────
function createDesktopButtons(desktopList) {
  desktopSection.innerHTML = "";
  floatDesktopBtns.innerHTML = "";

  desktopBtns = [];
  floatDskBtns = [];

  desktopList.forEach((num, index) => {
    // Insert separator before each desktop group after the first
    if (index > 0) {
      const sep = document.createElement("div");
      sep.className = "separator";
      desktopSection.appendChild(sep);
    }

    const container = document.createElement("div");
    container.className = "desktop-item-container";
    container.dataset.desktop = num;

    const btn = document.createElement("div");
    btn.className = "desktop-btn";
    btn.dataset.desktop = num;
    btn.textContent = num;
    container.appendChild(btn);
    desktopBtns.push(btn);

    const iconsDiv = document.createElement("div");
    iconsDiv.className = "desktop-icons";
    iconsDiv.id = `desktop-icons-${num}`;
    container.appendChild(iconsDiv);

    desktopSection.appendChild(container);

    const fbtn = document.createElement("div");
    fbtn.className = "float-desktop-btn";
    fbtn.dataset.desktop = num;
    fbtn.textContent = num;
    floatDesktopBtns.appendChild(fbtn);
    floatDskBtns.push(fbtn);
  });
}

// ─── Desktop Icons ────────────────────────────────────────────────────────
/**
 * UWP Square*Logo PNGs carry generous transparent padding around the artwork.
 * Detect the bounding box of non-transparent pixels and rescale the artwork
 * to fill the target size so the icon visually matches non-UWP icons.
 */
function cropAndResizeImage(dataUrl, targetSize) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = img.width;
        c.height = img.height;
        const ctx = c.getContext("2d");
        ctx.drawImage(img, 0, 0);
        const data = ctx.getImageData(0, 0, c.width, c.height);
        const px = data.data;
        let minX = c.width, minY = c.height, maxX = -1, maxY = -1;
        for (let y = 0; y < c.height; y++) {
          for (let x = 0; x < c.width; x++) {
            const a = px[(y * c.width + x) * 4 + 3];
            if (a > 8) {
              if (x < minX) minX = x;
              if (x > maxX) maxX = x;
              if (y < minY) minY = y;
              if (y > maxY) maxY = y;
            }
          }
        }
        if (maxX < 0) {
          resolve(dataUrl); // fully transparent — keep original
          return;
        }
        const cropW = maxX - minX + 1;
        const cropH = maxY - minY + 1;
        const out = document.createElement("canvas");
        out.width = targetSize;
        out.height = targetSize;
        const octx = out.getContext("2d");
        octx.imageSmoothingEnabled = true;
        octx.imageSmoothingQuality = "high";
        octx.drawImage(c, minX, minY, cropW, cropH, 0, 0, targetSize, targetSize);
        resolve(out.toDataURL("image/png"));
      } catch (e) {
        // CORS-tainted canvas, etc. — fall back to the original
        resolve(dataUrl);
      }
    };
    img.onerror = () => resolve(dataUrl);
    img.src = dataUrl;
  });
}

// Cache of the last rendered app list, used as a key for diff detection.
let _lastAppsKey = "";
let desktopAppOrders = {};
function _appsFingerprint(desktopApps) {
  // Signature: per-desktop list of hwnd+process_name+minimized, plus
  // currentDesktop so that desktop switches and minimize/restore changes
  // force a DOM rebuild and thus the grayscale filter gets updated.
  const parts = [`cd=${currentDesktop}`];
  for (const numStr of Object.keys(desktopApps).sort()) {
    const apps = desktopApps[numStr];
    const sig = apps
      .map((a) => `${a.hwnd}:${a.process_name}:${a.is_minimized ? "m" : "a"}`)
      .join(",");
    parts.push(`${numStr}=[${sig}]`);
  }
  return parts.join("|");
}

async function updateDesktopIcons() {
  try {
    const desktopApps = await invoke("get_desktop_apps");

    // Maintain stable icon order even when active window changes.
    for (const [numStr, apps] of Object.entries(desktopApps)) {
      if (!desktopAppOrders[numStr]) {
        desktopAppOrders[numStr] = [];
      }
      const currentProcessNames = apps.map((app) => app.process_name);

      // Remove apps that are no longer present on this desktop
      desktopAppOrders[numStr] = desktopAppOrders[numStr].filter((pName) =>
        currentProcessNames.includes(pName)
      );

      // Append newly discovered apps to the end
      currentProcessNames.forEach((pName) => {
        if (!desktopAppOrders[numStr].includes(pName)) {
          desktopAppOrders[numStr].push(pName);
        }
      });

      // Sort the apps based on the stable order
      apps.sort((a, b) => {
        return desktopAppOrders[numStr].indexOf(a.process_name) - desktopAppOrders[numStr].indexOf(b.process_name);
      });
    }

    // Diff against the previous poll: if nothing changed, do absolutely
    // nothing — no DOM rebuild, no Canvas re-encode, no SetWindowPos.
    // This eliminates the 2-second flicker that occurred because the
    // previous implementation rebuilt the entire icon tree on every tick.
    const fp = _appsFingerprint(desktopApps);
    if (fp === _lastAppsKey) return;
    _lastAppsKey = fp;

    // First, clear icons for all desktops (handles the case where ALL apps
    // on a desktop are gone and the Rust backend returns an empty HashMap,
    // i.e. the desktop key no longer exists in the response).
    for (const btn of desktopBtns) {
      const num = btn.dataset.desktop;
      const iconsDiv = document.getElementById(`desktop-icons-${num}`);
      if (iconsDiv) {
        iconsDiv.innerHTML = "";
      }
    }

    // Then re-populate icons for desktops that have apps
    for (const [numStr, apps] of Object.entries(desktopApps)) {
      const iconsDiv = document.getElementById(`desktop-icons-${numStr}`);
      if (iconsDiv) {
        for (const app of apps) {
          if (app.icon_base64) {
            const btn = document.createElement("div");
            btn.className = "desktop-app-btn";
            btn.title = app.process_name;
            btn.draggable = true;
            btn.addEventListener("dragstart", (e) => {
              btn.classList.add("dragging");
              taskbar.classList.add("dragging-active");
              e.dataTransfer.setData("text", app.hwnd.toString());
              e.dataTransfer.effectAllowed = "move";
            });
            btn.addEventListener("dragend", () => {
              btn.classList.remove("dragging");
              taskbar.classList.remove("dragging-active");
            });
            btn.addEventListener("click", (e) => {
              e.stopPropagation();
              invoke("focus_window", { hwnd: app.hwnd });
            });
            btn.addEventListener("auxclick", (e) => {
              if (e.button === 1) {
                e.stopPropagation();
                e.preventDefault();
                invoke("close_window", { hwnd: app.hwnd });
              }
            });

            const img = document.createElement("img");
            img.draggable = false;
            // UWP icons get their own class so CSS can size them larger
            // (UWP logos have transparent padding around the artwork).
            img.className = app.is_uwp ? "uwp-app-icon" : "desktop-app-icon";
            // For UWP icons: strip the transparent padding and rescale the
            // artwork to fill the box (button border − 1px).
            img.src = app.is_uwp
              ? await cropAndResizeImage(app.icon_base64, uwpIconSize)
              : app.icon_base64;
            // Grayscale: other desktops, minimized windows, or topmost windows on the current desktop
            const desktopNum = parseInt(numStr, 10);
            if (desktopNum !== currentDesktop || app.is_minimized || app.is_topmost) {
              img.style.filter = "grayscale(1)";
              img.style.opacity = "0.6";
            }
            btn.appendChild(img);

            iconsDiv.appendChild(btn);
          }
        }
      }
    }
    fitWindowToContent();
  } catch (e) {
    console.error("Failed to update desktop icons:", e);
  }
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
    loadTilingState();
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
///
/// Debounced + hysteretic: rapid calls are coalesced into a single
/// SetWindowPos, and small reflow churn (sub-pixel/icon-loading) is ignored
/// unless the difference exceeds `HYST_PX`. This prevents both the visible
/// flicker of repeated resizes and the transient WebView2 "WindowNotFound"
/// COM errors that occur when hwnd() is queried while a prior SetWindowPos
/// is still being processed.
const HYST_PX = 8;
let _fitTimer = null;
let _lastFitWidth = -1;
let _lastFitHeight = -1;
function fitWindowToContent(force = false) {
  if (_fitTimer !== null) clearTimeout(_fitTimer);
  _fitTimer = setTimeout(async () => {
    _fitTimer = null;
    const contentWidth = taskbar.scrollWidth;
    const height = config?.bar_height ?? 40;
    if (!force && _lastFitWidth >= 0 &&
      Math.abs(contentWidth - _lastFitWidth) < HYST_PX &&
      height === _lastFitHeight) {
      return; // no visible change; skip SetWindowPos
    }
    _lastFitWidth = contentWidth;
    _lastFitHeight = height;
    try {
      await invoke("set_window_size", { width: contentWidth, height: height });
    } catch (e) {
      // Suppress "Com_objects … WindowNotFound" noise; the next debounced
      // call will re-measure and resize if the value actually changed.
      console.debug("set_window_size:", e);
    }
  }, 150);
}

// ─── Taskbar Drag → Save Position ─────────────────────────────────────────
let moveDebounceTimer = null;
async function setupTaskbarMoveListener() {
  try {
    const window = getCurrentWindow();
    await window.onMoved((event) => {
      const pos = event.payload;
      if (moveDebounceTimer) clearTimeout(moveDebounceTimer);
      moveDebounceTimer = setTimeout(() => {
        invoke("set_float_pos", { x: pos.x, y: pos.y }).catch(() => { });
      }, 300);
    });
  } catch (e) {
    console.error("Failed to setup move listener:", e);
  }
}

// ─── Start ────────────────────────────────────────────────────────────────
document.addEventListener("DOMContentLoaded", () => {
  init();
  setupTaskbarMoveListener();
});

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
