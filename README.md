# 🪟 tile_wm

<img src="sample_taskbar.png" alt="tile_wm taskbar screenshot" width="700"/>

**tile_wm** is a custom top taskbar application for Windows 11.  
It provides intuitive virtual desktop switching and automatic window tiling with a simple UI.

Built with [Tauri v2](https://v2.tauri.app/) (Rust + Web frontend), it is lightweight and high-performance.

---

## Table of Contents

- [Main Features](#main-features)
  - [🖥️ Virtual Desktop Management](#️-virtual-desktop-management)
  - [📐 Window Tiling](#-window-tiling)
  - [🔄 Layout Cycle / Flip](#-layout-cycle--flip)
  - [📌 Float Mode](#-float-mode)
  - [🎯 Window Drag & Drop (Moving Between Virtual Desktops)](#-window-drag-drop-moving-between-virtual-desktops)
  - [⌨️ Hotkeys](#️-hotkeys)
  - [📋 Popup Menu](#-popup-menu)
  - [⚙️ Configuration File](#️-configuration-file)
- [Requirements](#requirements)
- [Build and Run](#build-and-run)
  - [Development Mode](#development-mode)
  - [Production Build](#production-build)
- [Configuration](#configuration)
  - [Configuration Items List](#configuration-items-list)
  - [Configuration Example](#configuration-example)
- [Project Structure](#project-structure)
- [Used Technologies](#used-technologies)
- [License](#license)

---

## Main Features

### 🖥️ Virtual Desktop Management

- Switch virtual desktops with a single click using buttons on the taskbar
- Follows real-time switching via Windows standard `Ctrl+Win+←/→`
- Monitors the registry (`HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops`) to accurately obtain the current desktop number
- Each virtual desktop maintains its own independent tiling mode

### 📐 Window Tiling

Automatically arranges windows on the active virtual desktop according to the selected layout.

| Mode | Icon | Layout |
|------|------|--------|
| **Free** | ![free](public/icons/free.png) | No tiling (free placement) |
| **2Win** | ![2Win](public/icons/2Win.png) | Left/right split (left main + right) ↔ (right main + left) |
| **3Win** | ![3Win](public/icons/3Win.png) | Left main + right upper + right lower ↕ Right main + left upper + left lower |
| **4Win** | ![4Win](public/icons/4Win.png) | 2×2 grid (main area position can be switched) |

- Sub-windows are automatically arranged relative to the main window
- Automatically detects window increases/decreases and re-applies tiling (with debounce processing)
- Possible to set window titles and process names to exclude from tiling

### 🔄 Layout Cycle / Flip

- **Cycle**: Clicking the same tiling mode button again swaps the main area position left/right or up/down (e.g., left main → right main)
- **Flip**: Holding `Ctrl` while clicking a 2Win/3Win/4Win button toggles the `flip_main` setting, flipping the main area orientation for all desktops

### 📌 Float Mode

- In Free mode, dragging the taskbar menu button (⋮) moves the taskbar to any position on the screen
- Float position is automatically saved to the configuration file and restored on next startup
- While floating, the window list and desktop switching functions remain available

### 🎯 Window Drag & Drop (Moving Between Virtual Desktops)

- Dragging an application icon on the taskbar to another desktop button moves that window to the destination virtual desktop
- Automatically switches to the destination desktop and focuses the moved window

### ⌨️ Hotkeys

Using a low-level keyboard hook (`WH_KEYBOARD_LL`), the following hotkeys are provided application-wide:

| Hotkey | Function |
|--------|----------|
| `Ctrl + Alt + Win + F12` | Move foreground window to the right desktop |
| `Ctrl + Alt + Win + F11` | Move foreground window to the left desktop |

After moving, automatically switches to the destination desktop and returns focus to the moved window.

### 📋 Popup Menu

Clicking the menu button (⋮) at the left end of the taskbar displays a context menu:

- **📝 Edit config.toml** — Open the configuration file in the default editor
- **❓ Help** — Show version information and a brief usage guide
- **Close menu** — Close the menu
- **Exit** — Exit the application

The menu automatically closes when the `Escape` key is pressed or focus is lost.

### ⚙️ Configuration File

- Automatically generated at `%LOCALAPPDATA%\tile_wm\config.toml`
- Appearance (colors, size), tiling parameters, spacing, etc. can be set in TOML format
- Configuration changes can be made by directly editing the file from the menu or via the app's settings UI for immediate reflection

---

## Requirements

| Item | Version / Requirement |
|------|-----------------------|
| **OS** | Windows 11 |
| **Rust** | 2021 Edition or later ([rustup](https://rustup.rs/) for installation) |
| **Node.js** | 18 or later ([Node.js](https://nodejs.org/)) |
| **npm** | 9 or later |

---

## Build and Run

### Development Mode

```bash
# Clone the repository
git clone https://github.com/yourusername/tile_wm.git
cd tile_wm

# Install frontend dependencies
npm install

# Start development server (hot-reload Tauri desktop app)
npm run tauri dev
```

### Production Build

```bash
# Build frontend and compile Rust binary
npm run tauri build
```

The executable is generated at `src-tauri/target/release/tile_wm.exe`.

> **Note**: On first build, Tauri system dependencies may need to be installed. See the [Tauri v2 Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) for details.

---

## Configuration

The configuration file is automatically generated at `%LOCALAPPDATA%\tile_wm\config.toml` on first launch.  
It can also be opened directly via the menu's **📝 Edit config.toml**.

### Configuration Items List

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `bar_height` | integer | `40` | Taskbar height (pixels) |
| `split_ratio_x` | integer | `50` | Horizontal split ratio (%) |
| `split_ratio_y` | integer | `50` | Vertical split ratio (%) |
| `exclude_titles` | string array | `[]` | List of window title substrings to exclude from tiling |
| `exclude_processes` | string array | `["tile_wm.exe"]` | List of process names to exclude from tiling |
| `window_x` | integer | `100` | Taskbar window X coordinate (float position) |
| `window_y` | integer | `100` | Taskbar window Y coordinate (float position) |
| `window_bg_rgba` | integer array | `[32, 32, 32, 255]` | Taskbar background color (RGBA) |
| `button_fg_rgb` | integer array | `[136, 136, 136]` | Button text color (RGB) |
| `button_bg_rgb` | integer array | `[32, 32, 32]` | Button background color (RGB) |
| `button_highlight_fg_rgb` | integer array | `[255, 255, 255]` | Button hover text color (RGB) |
| `button_highlight_bg_rgb` | integer array | `[255, 255, 255]` | Button hover background color (RGB) |
| `flip_main` | boolean | `false` | Flip main area orientation when tiling |
| `min_window_height` | integer | `220` | Minimum window height for tiling (pixels) |
| `top_spacing` | integer | `40` | Top screen margin (taskbar height) |
| `bottom_spacing` | integer | `10` | Bottom screen margin |
| `left_spacing` | integer | `10` | Left screen margin |
| `right_spacing` | integer | `10` | Right screen margin |
| `inner_spacing` | integer | `10` | Inner spacing between windows when tiling |

> **Supplementary**: Keys in the configuration file should be written in snake_case.

### Configuration Example

```toml
bar_height = 36
split_ratio_x = 60
split_ratio_y = 50
exclude_titles = ["Calculator", "Settings"]
exclude_processes = ["tile_wm.exe"]
window_x = 200
window_y = 50
window_bg_rgba = [48, 48, 48, 255]
button_fg_rgb = [180, 180, 180]
button_bg_rgb = [48, 48, 48]
button_highlight_fg_rgb = [255, 255, 255]
button_highlight_bg_rgb = [64, 120, 242]
flip_main = false
min_window_height = 220
top_spacing = 40
bottom_spacing = 10
left_spacing = 10
right_spacing = 10
inner_spacing = 6
```

---

## Project Structure

```
tile_wm/
├── index.html                  # Main window (taskbar) HTML
├── menu.html                   # Popup menu HTML
├── package.json                # Frontend dependencies
├── vite.config.ts              # Vite build configuration
├── specification.md            # Design specification
├── sample_taskbar.png          # Screenshot
├── public/
│   └── icons/                  # Tiling mode icons
│       ├── free.png
│       ├── 2Win.png / 2Win-R.png
│       ├── 3Win.png / 3Win-R.png
│       └── 4Win.png / 4Win-R.png
├── src/                        # Frontend (JavaScript / CSS)
│   ├── main.js                 # Main window logic
│   ├── menu.js                 # Popup menu logic
│   ├── styles.css              # Taskbar styles
│   └── menu.css                # Menu styles
└── src-tauri/                  # Rust backend
    ├── Cargo.toml              # Rust dependencies
    ├── tauri.conf.json         # Tauri configuration
    ├── icons/                  # App icons
    └── src/
        ├── main.rs             # Entry point
        ├── lib.rs              # App initialization & module management
        ├── config.rs           # Configuration file read/write
        ├── app_bar.rs          # Windows AppBar registration & window placement
        ├── desktop.rs          # Virtual desktop management (registry / COM / winvd)
        ├── tiling.rs           # Tiling layout calculation engine
        ├── commands.rs         # Tauri IPC commands (frontend ↔ backend)
        ├── hotkey.rs           # Global hotkeys (WH_KEYBOARD_LL)
        └── win_event.rs        # Window increase/decrease detection & automatic tiling
```

---

## Used Technologies

| Category | Technology |
|----------|------------|
| **Frontend** | [Vite](https://vitejs.dev/) + Vanilla JS + CSS |
| **Desktop Framework** | [Tauri v2](https://v2.tauri.app/) |
| **Backend** | [Rust](https://www.rust-lang.org/) 2021 Edition |
| **Windows API** | [windows-rs](https://github.com/microsoft/windows-rs) 0.58 |
| **Serialization** | [serde](https://serde.rs/) / [toml](https://github.com/toml-rs/toml) |
| **Virtual Desktop API** | [winvd](https://crates.io/crates/winvd) 0.0.49 |
| **Logging** | [log](https://crates.io/crates/log) |

---

## License

[MIT](./LICENSE)

---

> **tile_wm** — Making desktop management on Windows 11 more comfortable.