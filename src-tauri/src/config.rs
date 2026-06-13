use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bar_height: i32,
    pub split_ratio_x: i32,
    pub split_ratio_y: i32,
    pub exclude_titles: Vec<String>,
    pub exclude_processes: Vec<String>,
    pub window_x: i32,
    pub window_y: i32,
    pub window_bg_rgba: [u8; 4],
    pub button_fg_rgb: [u8; 3],
    pub button_bg_rgb: [u8; 3],
    pub button_highlight_fg_rgb: [u8; 3],
    pub button_highlight_bg_rgb: [u8; 3],
    pub flip_main: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bar_height: 40,
            split_ratio_x: 50,
            split_ratio_y: 50,
            exclude_titles: vec![],
            exclude_processes: vec!["tile_wm.exe".to_string()],
            window_x: 100,
            window_y: 100,
            window_bg_rgba: [32, 32, 32, 255],
            button_fg_rgb: [136, 136, 136],
            button_bg_rgb: [32, 32, 32],
            button_highlight_fg_rgb: [255, 255, 255],
            button_highlight_bg_rgb: [255, 255, 255],
            flip_main: false,
        }
    }
}

pub fn config_path() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE")
                .unwrap_or_else(|_| r"C:\Users\Default".to_string());
            PathBuf::from(home).join("AppData").join("Local").to_string_lossy().into_owned()
        });
    let dir = PathBuf::from(local_app_data).join("tile_wm");
    fs::create_dir_all(&dir).ok();
    dir.join("config.toml")
}

pub fn load_config() -> Config {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    } else {
        let cfg = Config::default();
        save_config(&cfg);
        cfg
    }
}

pub fn save_config(config: &Config) {
    let path = config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        fs::write(&path, content).ok();
    }
}
