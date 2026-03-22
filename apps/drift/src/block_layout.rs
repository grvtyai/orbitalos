use serde::{Deserialize, Serialize};

pub const CANVAS_WIDTH: i32 = 1600;
pub const CANVAS_HEIGHT: i32 = 1200;
pub const GRID_SIZE: i32 = 8;

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
        Self {
            text_block: TextBlockLayout {
                x: GRID_SIZE * 3,
                y: GRID_SIZE * 3,
                width: GRID_SIZE * 68,
                height: GRID_SIZE * 48,
            },
        }
    }
}

impl TextBlockLayout {
    pub fn preview_constrained(mut self) -> Self {
        self.width = self.width.max(GRID_SIZE * 8);
        self.height = self.height.max(GRID_SIZE * 8);
        self
    }

    pub fn snapped_to_grid(mut self) -> Self {
        let snapped_left = snap_value(self.x);
        let snapped_top = snap_value(self.y);
        let snapped_right = snap_value(self.x + self.width);
        let snapped_bottom = snap_value(self.y + self.height);

        self.x = snapped_left;
        self.y = snapped_top;
        self.width = (snapped_right - snapped_left).max(GRID_SIZE * 8);
        self.height = (snapped_bottom - snapped_top).max(GRID_SIZE * 8);
        self
    }

    pub fn clamp_to_canvas(mut self) -> Self {
        self.width = self.width.clamp(GRID_SIZE * 8, CANVAS_WIDTH - GRID_SIZE);
        self.height = self.height.clamp(GRID_SIZE * 8, CANVAS_HEIGHT - GRID_SIZE);
        self.x = self.x.clamp(0, CANVAS_WIDTH - self.width);
        self.y = self.y.clamp(0, CANVAS_HEIGHT - self.height);
        self
    }
}

pub fn serialize_layout(layout: &NoteCanvasLayout) -> Option<String> {
    serde_json::to_string(layout).ok()
}

pub fn deserialize_layout(value: Option<&str>) -> NoteCanvasLayout {
    value
        .and_then(|json| serde_json::from_str::<NoteCanvasLayout>(json).ok())
        .unwrap_or_default()
}

fn snap_value(value: i32) -> i32 {
    let base = (value as f64 / GRID_SIZE as f64).round() as i32;
    base * GRID_SIZE
}
