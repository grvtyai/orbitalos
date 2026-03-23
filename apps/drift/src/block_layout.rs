use serde::{Deserialize, Serialize};

pub const CANVAS_WIDTH: i32 = 1600;
pub const CANVAS_HEIGHT: i32 = 1200;
pub const DEFAULT_GRID_SIZE: i32 = 8;

const MIN_BLOCK_UNITS: i32 = 8;
const DEFAULT_BLOCK_X_UNITS: i32 = 3;
const DEFAULT_BLOCK_Y_UNITS: i32 = 3;
const DEFAULT_BLOCK_WIDTH_UNITS: i32 = 68;
const DEFAULT_BLOCK_HEIGHT_UNITS: i32 = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteCanvasLayout {
    pub text_block: TextBlockLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBlockLayout {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for NoteCanvasLayout {
    fn default() -> Self {
        default_note_canvas_layout(DEFAULT_GRID_SIZE)
    }
}

impl TextBlockLayout {
    pub fn preview_constrained(mut self, grid_size: i32) -> Self {
        let min_size = normalized_grid_size(grid_size) * MIN_BLOCK_UNITS;
        self.width = self.width.max(min_size);
        self.height = self.height.max(min_size);
        self
    }

    pub fn snapped_to_grid(mut self, grid_size: i32) -> Self {
        let grid_size = normalized_grid_size(grid_size);
        let snapped_left = snap_value(self.x, grid_size);
        let snapped_top = snap_value(self.y, grid_size);
        let snapped_right = snap_value(self.x + self.width, grid_size);
        let snapped_bottom = snap_value(self.y + self.height, grid_size);

        self.x = snapped_left;
        self.y = snapped_top;
        self.width = (snapped_right - snapped_left).max(grid_size * MIN_BLOCK_UNITS);
        self.height = (snapped_bottom - snapped_top).max(grid_size * MIN_BLOCK_UNITS);
        self
    }

    pub fn clamp_to_canvas(mut self, grid_size: i32) -> Self {
        let grid_size = normalized_grid_size(grid_size);
        self.width = self
            .width
            .clamp(grid_size * MIN_BLOCK_UNITS, CANVAS_WIDTH - grid_size);
        self.height = self
            .height
            .clamp(grid_size * MIN_BLOCK_UNITS, CANVAS_HEIGHT - grid_size);
        self.x = self.x.clamp(0, CANVAS_WIDTH - self.width);
        self.y = self.y.clamp(0, CANVAS_HEIGHT - self.height);
        self
    }
}

pub fn default_note_canvas_layout(grid_size: i32) -> NoteCanvasLayout {
    let grid_size = normalized_grid_size(grid_size);

    NoteCanvasLayout {
        text_block: TextBlockLayout {
            x: grid_size * DEFAULT_BLOCK_X_UNITS,
            y: grid_size * DEFAULT_BLOCK_Y_UNITS,
            width: grid_size * DEFAULT_BLOCK_WIDTH_UNITS,
            height: grid_size * DEFAULT_BLOCK_HEIGHT_UNITS,
        },
    }
}

pub fn serialize_layout(layout: &NoteCanvasLayout) -> Option<String> {
    serde_json::to_string(layout).ok()
}

pub fn deserialize_layout(value: Option<&str>, grid_size: i32) -> NoteCanvasLayout {
    value
        .and_then(|json| serde_json::from_str::<NoteCanvasLayout>(json).ok())
        .unwrap_or_else(|| default_note_canvas_layout(grid_size))
}

fn snap_value(value: i32, grid_size: i32) -> i32 {
    let base = (value as f64 / grid_size as f64).round() as i32;
    base * grid_size
}

fn normalized_grid_size(grid_size: i32) -> i32 {
    grid_size.max(1)
}
