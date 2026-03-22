use gtk::gdk;
use gtk::prelude::*;
use gtk::{pango, TextBuffer, TextTagTable};
use serde::{Deserialize, Serialize};

const TAG_BOLD: &str = "format-bold";
const TAG_ITALIC: &str = "format-italic";
const TAG_UNDERLINE: &str = "format-underline";
const TAG_STRIKETHROUGH: &str = "format-strikethrough";
const TAG_COLOR_RED: &str = "format-color-red";
const TAG_COLOR_BLUE: &str = "format-color-blue";
const TAG_COLOR_GREEN: &str = "format-color-green";
const TAG_COLOR_ORANGE: &str = "format-color-orange";

const COLOR_TAGS: &[(&str, &str)] = &[
    (TAG_COLOR_RED, "red"),
    (TAG_COLOR_BLUE, "blue"),
    (TAG_COLOR_GREEN, "green"),
    (TAG_COLOR_ORANGE, "orange"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RichTextSpan {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFormat {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub color: Option<String>,
}

impl TextStyle {
    fn plain() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            color: None,
        }
    }
}

impl PendingFormat {
    pub fn is_plain(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && !self.strikethrough
            && self.color.is_none()
    }
}

pub fn create_buffer() -> TextBuffer {
    let tag_table = TextTagTable::new();
    let buffer = TextBuffer::new(Some(&tag_table));

    buffer.create_tag(Some(TAG_BOLD), &[("weight", &700i32)]);
    buffer.create_tag(Some(TAG_ITALIC), &[("style", &pango::Style::Italic)]);
    buffer.create_tag(
        Some(TAG_UNDERLINE),
        &[("underline", &pango::Underline::Single)],
    );
    buffer.create_tag(
        Some(TAG_STRIKETHROUGH),
        &[("strikethrough", &true)],
    );
    buffer.create_tag(
        Some(TAG_COLOR_RED),
        &[("foreground-rgba", &gdk::RGBA::new(0.78, 0.11, 0.16, 1.0))],
    );
    buffer.create_tag(
        Some(TAG_COLOR_BLUE),
        &[("foreground-rgba", &gdk::RGBA::new(0.11, 0.33, 0.84, 1.0))],
    );
    buffer.create_tag(
        Some(TAG_COLOR_GREEN),
        &[("foreground-rgba", &gdk::RGBA::new(0.16, 0.52, 0.18, 1.0))],
    );
    buffer.create_tag(
        Some(TAG_COLOR_ORANGE),
        &[("foreground-rgba", &gdk::RGBA::new(0.86, 0.43, 0.11, 1.0))],
    );

    buffer
}

pub fn set_buffer_content(buffer: &TextBuffer, plain_text: &str, markup: Option<&str>) {
    buffer.set_text("");

    if let Some(markup) = markup {
        if let Ok(spans) = serde_json::from_str::<Vec<RichTextSpan>>(markup) {
            if !spans.is_empty() {
                insert_spans(buffer, &spans);
                return;
            }
        }
    }

    buffer.set_text(plain_text);
}

pub fn serialize_buffer(buffer: &TextBuffer) -> Option<String> {
    let spans = collect_spans(buffer);

    if spans.is_empty() {
        return None;
    }

    if spans
        .iter()
        .all(|span| !span.bold && !span.italic && !span.underline && !span.strikethrough && span.color.is_none())
    {
        return None;
    }

    serde_json::to_string(&spans).ok()
}

pub fn set_bold(buffer: &TextBuffer, active: bool) -> bool {
    set_named_tag(buffer, TAG_BOLD, active)
}

pub fn set_italic(buffer: &TextBuffer, active: bool) -> bool {
    set_named_tag(buffer, TAG_ITALIC, active)
}

pub fn set_underline(buffer: &TextBuffer, active: bool) -> bool {
    set_named_tag(buffer, TAG_UNDERLINE, active)
}

pub fn set_strikethrough(buffer: &TextBuffer, active: bool) -> bool {
    set_named_tag(buffer, TAG_STRIKETHROUGH, active)
}

pub fn clear_formatting(buffer: &TextBuffer) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    buffer.remove_tag_by_name(TAG_BOLD, &start, &end);
    buffer.remove_tag_by_name(TAG_ITALIC, &start, &end);
    buffer.remove_tag_by_name(TAG_UNDERLINE, &start, &end);
    buffer.remove_tag_by_name(TAG_STRIKETHROUGH, &start, &end);
    remove_color_tags(buffer, &start, &end);
    true
}

pub fn set_color(buffer: &TextBuffer, color_name: Option<&str>) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    remove_color_tags(buffer, &start, &end);

    if let Some(color_name) = color_name {
        let Some((tag_name, _)) = COLOR_TAGS.iter().find(|(_, color)| *color == color_name) else {
            return false;
        };

        buffer.apply_tag_by_name(tag_name, &start, &end);
    }

    true
}

pub fn apply_pending_format_by_offsets(
    buffer: &TextBuffer,
    start_offset: i32,
    char_count: i32,
    format: &PendingFormat,
) {
    if char_count <= 0 {
        return;
    }

    let safe_start = start_offset.clamp(0, buffer.char_count());
    let safe_end = (safe_start + char_count).clamp(safe_start, buffer.char_count());

    let start = buffer.iter_at_offset(safe_start);
    let end = buffer.iter_at_offset(safe_end);
    clear_style_range(buffer, &start, &end);

    if format.bold {
        buffer.apply_tag_by_name(TAG_BOLD, &start, &end);
    }

    if format.italic {
        buffer.apply_tag_by_name(TAG_ITALIC, &start, &end);
    }

    if format.underline {
        buffer.apply_tag_by_name(TAG_UNDERLINE, &start, &end);
    }

    if format.strikethrough {
        buffer.apply_tag_by_name(TAG_STRIKETHROUGH, &start, &end);
    }

    if let Some(color) = &format.color {
        if let Some((tag_name, _)) = COLOR_TAGS.iter().find(|(_, value)| value == color) {
            buffer.apply_tag_by_name(tag_name, &start, &end);
        }
    }
}

fn set_named_tag(buffer: &TextBuffer, tag_name: &str, active: bool) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    if active {
        buffer.apply_tag_by_name(tag_name, &start, &end);
    } else {
        buffer.remove_tag_by_name(tag_name, &start, &end);
    }

    true
}

fn clear_style_range(buffer: &TextBuffer, start: &gtk::TextIter, end: &gtk::TextIter) {
    buffer.remove_tag_by_name(TAG_BOLD, start, end);
    buffer.remove_tag_by_name(TAG_ITALIC, start, end);
    buffer.remove_tag_by_name(TAG_UNDERLINE, start, end);
    buffer.remove_tag_by_name(TAG_STRIKETHROUGH, start, end);
    remove_color_tags(buffer, start, end);
}

fn remove_color_tags(buffer: &TextBuffer, start: &gtk::TextIter, end: &gtk::TextIter) {
    for (tag_name, _) in COLOR_TAGS {
        buffer.remove_tag_by_name(tag_name, start, end);
    }
}

fn insert_spans(buffer: &TextBuffer, spans: &[RichTextSpan]) {
    for span in spans {
        let start_offset = buffer.char_count();
        let mut insert_at = buffer.end_iter();
        buffer.insert(&mut insert_at, &span.text);
        let end_offset = buffer.char_count();
        let start = buffer.iter_at_offset(start_offset);
        let end = buffer.iter_at_offset(end_offset);

        if span.bold {
            buffer.apply_tag_by_name(TAG_BOLD, &start, &end);
        }

        if span.italic {
            buffer.apply_tag_by_name(TAG_ITALIC, &start, &end);
        }

        if span.underline {
            buffer.apply_tag_by_name(TAG_UNDERLINE, &start, &end);
        }

        if span.strikethrough {
            buffer.apply_tag_by_name(TAG_STRIKETHROUGH, &start, &end);
        }

        if let Some(color) = &span.color {
            if let Some((tag_name, _)) = COLOR_TAGS.iter().find(|(_, value)| value == color) {
                buffer.apply_tag_by_name(tag_name, &start, &end);
            }
        }
    }
}

fn collect_spans(buffer: &TextBuffer) -> Vec<RichTextSpan> {
    let end = buffer.end_iter();
    let mut iter = buffer.start_iter();
    let mut spans = Vec::new();
    let mut current_style = TextStyle::plain();
    let mut current_text = String::new();
    let mut initialized = false;

    while iter != end {
        let style = style_at_iter(buffer, &iter);
        let mut next = iter.clone();

        if !next.forward_char() {
            break;
        }

        let text = buffer.text(&iter, &next, true).to_string();

        if !initialized {
            current_style = style.clone();
            initialized = true;
        }

        if style != current_style {
            spans.push(RichTextSpan {
                text: current_text,
                bold: current_style.bold,
                italic: current_style.italic,
                underline: current_style.underline,
                strikethrough: current_style.strikethrough,
                color: current_style.color.clone(),
            });
            current_text = String::new();
            current_style = style;
        }

        current_text.push_str(&text);
        iter = next;
    }

    if !current_text.is_empty() {
        spans.push(RichTextSpan {
            text: current_text,
            bold: current_style.bold,
            italic: current_style.italic,
            underline: current_style.underline,
            strikethrough: current_style.strikethrough,
            color: current_style.color.clone(),
        });
    }

    spans
        .into_iter()
        .filter(|span| !span.text.is_empty())
        .collect()
}

fn style_at_iter(buffer: &TextBuffer, iter: &gtk::TextIter) -> TextStyle {
    let mut style = TextStyle::plain();

    if let Some(tag) = buffer.tag_table().lookup(TAG_BOLD) {
        style.bold = iter.has_tag(&tag);
    }

    if let Some(tag) = buffer.tag_table().lookup(TAG_ITALIC) {
        style.italic = iter.has_tag(&tag);
    }

    if let Some(tag) = buffer.tag_table().lookup(TAG_UNDERLINE) {
        style.underline = iter.has_tag(&tag);
    }

    if let Some(tag) = buffer.tag_table().lookup(TAG_STRIKETHROUGH) {
        style.strikethrough = iter.has_tag(&tag);
    }

    for (tag_name, color) in COLOR_TAGS {
        if let Some(tag) = buffer.tag_table().lookup(tag_name) {
            if iter.has_tag(&tag) {
                style.color = Some((*color).to_string());
                break;
            }
        }
    }

    style
}
