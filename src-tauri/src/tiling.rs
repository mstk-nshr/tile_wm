use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TilingMode {
    #[serde(rename = "free")]
    Free,
    #[serde(rename = "2windows")]
    TwoWindows,
    #[serde(rename = "3windows")]
    ThreeWindows,
    #[serde(rename = "4windows")]
    FourWindows,
}

impl TilingMode {
    pub fn next(&self) -> Self {
        match self {
            TilingMode::Free => TilingMode::TwoWindows,
            TilingMode::TwoWindows => TilingMode::ThreeWindows,
            TilingMode::ThreeWindows => TilingMode::FourWindows,
            TilingMode::FourWindows => TilingMode::Free,
        }
    }
}

/// Represents a tiled window region
#[derive(Debug, Clone)]
pub struct TileRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Calculate tile regions for a given monitor work area and tiling mode
pub struct TilingConfig {
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub monitor_w: i32,
    pub monitor_h: i32,
    pub split_ratio_x: i32,
    pub split_ratio_y: i32,
    pub flip_main: bool,
    pub top_spacing: i32,
    pub bottom_spacing: i32,
    pub left_spacing: i32,
    pub right_spacing: i32,
    pub inner_spacing: i32,
}

pub fn calculate_tiles(
    mode: TilingMode,
    config: TilingConfig,
    window_count: usize,
) -> Vec<TileRegion> {
    // Work area with outer spacing applied
    let work_x = config.monitor_x + config.left_spacing;
    let work_y = config.monitor_y + config.top_spacing;
    let work_w = config.monitor_w - config.left_spacing - config.right_spacing;
    let work_h = config.monitor_h - config.top_spacing - config.bottom_spacing;

    let mut tiles = match mode {
        TilingMode::Free => vec![],

        TilingMode::TwoWindows => {
            let ratio = config.split_ratio_x as f64 / 100.0;
            // Available width after subtracting inner spacing gap between left and right
            let avail_w = work_w - config.inner_spacing;
            let left_w = (avail_w as f64 * ratio) as i32;
            if window_count <= 1 {
                vec![TileRegion {
                    x: work_x,
                    y: work_y,
                    width: left_w,
                    height: work_h,
                }]
            } else {
                let right_w = avail_w - left_w;
                vec![
                    TileRegion {
                        x: work_x,
                        y: work_y,
                        width: left_w,
                        height: work_h,
                    },
                    TileRegion {
                        x: work_x + left_w + config.inner_spacing,
                        y: work_y,
                        width: right_w,
                        height: work_h,
                    },
                ]
            }
        }

        TilingMode::ThreeWindows => {
            let ratio_x = config.split_ratio_x as f64 / 100.0;
            let avail_w = work_w - config.inner_spacing;
            let left_w = (avail_w as f64 * ratio_x) as i32;
            if window_count <= 1 {
                vec![TileRegion {
                    x: work_x,
                    y: work_y,
                    width: left_w,
                    height: work_h,
                }]
            } else if window_count == 2 {
                // 3Win identity: main (left, full-height) + stack top (right, half-height)
                let right_w = avail_w - left_w;
                let avail_h = work_h - config.inner_spacing;
                let right_h = avail_h / 2;
                vec![
                    TileRegion {
                        x: work_x,
                        y: work_y,
                        width: left_w,
                        height: work_h,
                    },
                    TileRegion {
                        x: work_x + left_w + config.inner_spacing,
                        y: work_y,
                        width: right_w,
                        height: right_h,
                    },
                ]
            } else {
                // Left main + right stack (top/bottom)
                let right_w = avail_w - left_w;
                let avail_h = work_h - config.inner_spacing;
                let right_h = avail_h / 2;
                vec![
                    TileRegion {
                        x: work_x,
                        y: work_y,
                        width: left_w,
                        height: work_h,
                    },
                    TileRegion {
                        x: work_x + left_w + config.inner_spacing,
                        y: work_y,
                        width: right_w,
                        height: right_h,
                    },
                    TileRegion {
                        x: work_x + left_w + config.inner_spacing,
                        y: work_y + right_h + config.inner_spacing,
                        width: right_w,
                        height: avail_h - right_h,
                    },
                ]
            }
        }

        TilingMode::FourWindows => {
            let ratio_x = config.split_ratio_x as f64 / 100.0;
            let ratio_y = config.split_ratio_y as f64 / 100.0;
            let avail_w = work_w - config.inner_spacing;
            let avail_h = work_h - config.inner_spacing;
            let left_w = (avail_w as f64 * ratio_x) as i32;
            let top_h = (avail_h as f64 * ratio_y) as i32;

            match window_count {
                0..=1 => {
                    vec![TileRegion {
                        x: work_x,
                        y: work_y,
                        width: left_w,
                        height: top_h,
                    }]
                }
                2 => {
                    // 4Win identity: top-left (main) + top-right (quadrant row)
                    let right_w = avail_w - left_w;
                    vec![
                        TileRegion {
                            x: work_x,
                            y: work_y,
                            width: left_w,
                            height: top_h,
                        },
                        TileRegion {
                            x: work_x + left_w + config.inner_spacing,
                            y: work_y,
                            width: right_w,
                            height: top_h,
                        },
                    ]
                }
                3 => {
                    // 4Win identity: top-left (main) + top-right + bottom-left (L-shape)
                    let right_w = avail_w - left_w;
                    vec![
                        TileRegion {
                            x: work_x,
                            y: work_y,
                            width: left_w,
                            height: top_h,
                        },
                        TileRegion {
                            x: work_x + left_w + config.inner_spacing,
                            y: work_y,
                            width: right_w,
                            height: top_h,
                        },
                        TileRegion {
                            x: work_x,
                            y: work_y + top_h + config.inner_spacing,
                            width: left_w,
                            height: avail_h - top_h,
                        },
                    ]
                }
                _ => {
                    // 4+ windows: 2x2 grid
                    let right_w = avail_w - left_w;
                    vec![
                        TileRegion {
                            x: work_x,
                            y: work_y,
                            width: left_w,
                            height: top_h,
                        },
                        TileRegion {
                            x: work_x + left_w + config.inner_spacing,
                            y: work_y,
                            width: right_w,
                            height: top_h,
                        },
                        TileRegion {
                            x: work_x,
                            y: work_y + top_h + config.inner_spacing,
                            width: left_w,
                            height: avail_h - top_h,
                        },
                        TileRegion {
                            x: work_x + left_w + config.inner_spacing,
                            y: work_y + top_h + config.inner_spacing,
                            width: right_w,
                            height: avail_h - top_h,
                        },
                    ]
                }
            }
        }
    };

    // Apply horizontal flip if enabled — mirror all tiles across the monitor center
    if config.flip_main {
        for tile in &mut tiles {
            let right = tile.x + tile.width;
            tile.x = config.monitor_x + (config.monitor_w - (right - config.monitor_x));
        }
    }

    tiles
}