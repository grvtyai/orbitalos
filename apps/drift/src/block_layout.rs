use serde::{Deserialize, Serialize};

pub const CANVAS_WIDTH: i32 = 1600;
pub const CANVAS_HEIGHT: i32 = 1200;
pub const DEFAULT_GRID_SIZE: i32 = 8;

const MIN_TEXT_BLOCK_UNITS: i32 = 8;
const MIN_CODE_BLOCK_UNITS: i32 = 1;
const DEFAULT_BLOCK_X_UNITS: i32 = 3;
const DEFAULT_BLOCK_Y_UNITS: i32 = 3;
const DEFAULT_BLOCK_WIDTH_UNITS: i32 = 68;
const DEFAULT_BLOCK_HEIGHT_UNITS: i32 = 48;
const DEFAULT_CODE_BLOCK_WIDTH_UNITS: i32 = 52;
const DEFAULT_CODE_BLOCK_HEIGHT_UNITS: i32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteCanvasLayout {
    pub blocks: Vec<TextBlockState>,
    pub active_block_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Text,
    Code,
}

impl Default for BlockKind {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBlockState {
    #[serde(default)]
    pub kind: BlockKind,
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub body_markup: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LegacyNoteCanvasLayout {
    text_block: LegacyTextBlockLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LegacyTextBlockLayout {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Default for NoteCanvasLayout {
    fn default() -> Self {
        default_note_canvas_layout(DEFAULT_GRID_SIZE)
    }
}

impl TextBlockState {
    pub fn layout(&self) -> TextBlockLayout {
        TextBlockLayout {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn with_layout(mut self, layout: TextBlockLayout) -> Self {
        self.x = layout.x;
        self.y = layout.y;
        self.width = layout.width;
        self.height = layout.height;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlockLayout {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl TextBlockLayout {
    pub fn preview_constrained_with_min_units(mut self, grid_size: i32, min_units: i32) -> Self {
        let min_size = normalized_grid_size(grid_size) * min_units.max(1);
        self.width = self.width.max(min_size);
        self.height = self.height.max(min_size);
        self
    }

    pub fn snapped_to_grid_with_min_units(mut self, grid_size: i32, min_units: i32) -> Self {
        let grid_size = normalized_grid_size(grid_size);
        let snapped_left = snap_value(self.x, grid_size);
        let snapped_top = snap_value(self.y, grid_size);
        let snapped_right = snap_value(self.x + self.width, grid_size);
        let snapped_bottom = snap_value(self.y + self.height, grid_size);

        self.x = snapped_left;
        self.y = snapped_top;
        self.width = (snapped_right - snapped_left).max(grid_size * min_units.max(1));
        self.height = (snapped_bottom - snapped_top).max(grid_size * min_units.max(1));
        self
    }

    pub fn clamp_to_canvas_with_min_units(mut self, grid_size: i32, min_units: i32) -> Self {
        let grid_size = normalized_grid_size(grid_size);
        let min_size = grid_size * min_units.max(1);
        self.width = self
            .width
            .clamp(min_size, CANVAS_WIDTH - grid_size);
        self.height = self
            .height
            .clamp(min_size, CANVAS_HEIGHT - grid_size);
        self.x = self.x.clamp(0, CANVAS_WIDTH - self.width);
        self.y = self.y.clamp(0, CANVAS_HEIGHT - self.height);
        self
    }
}

pub fn min_block_units(kind: BlockKind) -> i32 {
    match kind {
        BlockKind::Text => MIN_TEXT_BLOCK_UNITS,
        BlockKind::Code => MIN_CODE_BLOCK_UNITS,
    }
}

pub fn default_note_canvas_layout(grid_size: i32) -> NoteCanvasLayout {
    let block = default_text_block_state("text-block-1".to_string(), grid_size);

    NoteCanvasLayout {
        active_block_id: Some(block.id.clone()),
        blocks: vec![block],
    }
}

pub fn default_text_block_state(id: String, grid_size: i32) -> TextBlockState {
    let grid_size = normalized_grid_size(grid_size);

    TextBlockState {
        kind: BlockKind::Text,
        id,
        x: grid_size * DEFAULT_BLOCK_X_UNITS,
        y: grid_size * DEFAULT_BLOCK_Y_UNITS,
        width: grid_size * DEFAULT_BLOCK_WIDTH_UNITS,
        height: grid_size * DEFAULT_BLOCK_HEIGHT_UNITS,
        body: String::new(),
        body_markup: None,
    }
}

pub fn default_code_block_state(id: String, grid_size: i32) -> TextBlockState {
    let grid_size = normalized_grid_size(grid_size);

    TextBlockState {
        kind: BlockKind::Code,
        id,
        x: grid_size * DEFAULT_BLOCK_X_UNITS,
        y: grid_size * DEFAULT_BLOCK_Y_UNITS,
        width: grid_size * DEFAULT_CODE_BLOCK_WIDTH_UNITS,
        height: grid_size * DEFAULT_CODE_BLOCK_HEIGHT_UNITS,
        body: String::new(),
        body_markup: None,
    }
}

pub fn serialize_layout(layout: &NoteCanvasLayout) -> Option<String> {
    serde_json::to_string(layout).ok()
}

pub fn deserialize_layout(
    value: Option<&str>,
    grid_size: i32,
    fallback_body: &str,
    fallback_body_markup: Option<&str>,
) -> NoteCanvasLayout {
    if let Some(json) = value {
        if let Ok(layout) = serde_json::from_str::<NoteCanvasLayout>(json) {
            let mut layout = layout;

            if layout.blocks.is_empty() {
                return default_note_canvas_layout(grid_size);
            }

            if layout.active_block_id.is_none() {
                layout.active_block_id = layout.blocks.first().map(|block| block.id.clone());
            }

            return layout;
        }

        if let Ok(legacy) = serde_json::from_str::<LegacyNoteCanvasLayout>(json) {
            return NoteCanvasLayout {
                active_block_id: Some("text-block-1".to_string()),
                blocks: vec![TextBlockState {
                    kind: BlockKind::Text,
                    id: "text-block-1".to_string(),
                    x: legacy.text_block.x,
                    y: legacy.text_block.y,
                    width: legacy.text_block.width,
                    height: legacy.text_block.height,
                    body: fallback_body.to_string(),
                    body_markup: fallback_body_markup.map(ToString::to_string),
                }],
            };
        }
    }

    let mut layout = default_note_canvas_layout(grid_size);
    if let Some(block) = layout.blocks.first_mut() {
        block.body = fallback_body.to_string();
        block.body_markup = fallback_body_markup.map(ToString::to_string);
    }
    layout
}

pub fn compose_note_body(layout: &NoteCanvasLayout) -> String {
    layout
        .blocks
        .iter()
        .map(|block| block.body.trim())
        .filter(|body| !body.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn snap_value(value: i32, grid_size: i32) -> i32 {
    let base = (value as f64 / grid_size as f64).round() as i32;
    base * grid_size
}

fn normalized_grid_size(grid_size: i32) -> i32 {
    grid_size.max(1)
}
