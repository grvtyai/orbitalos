use gtk::gdk;
use gtk::prelude::*;
use gtk::{pango, TextBuffer, TextTagTable};
use serde::{Deserialize, Serialize};

const TAG_BOLD: &str = "format-bold";
const TAG_UNDERLINE: &str = "format-underline";
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
    underline: bool,
    color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextStyle {
    bold: bool,
    underline: bool,
    color: Option<String>,
}

impl TextStyle {
    fn plain() -> Self {
        Self {
            bold: false,
            underline: false,
            color: None,
        }
    }
}

pub fn create_buffer() -> TextBuffer {
    let tag_table = TextTagTable::new();
    let buffer = TextBuffer::new(Some(&tag_table));

    buffer.create_tag(Some(TAG_BOLD), &[("weight", &700i32)]);
    buffer.create_tag(
        Some(TAG_UNDERLINE),
        &[("underline", &pango::Underline::Single)],
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

    if spans.iter().all(|span| {
        !span.bold && !span.underline && span.color.is_none()
    }) {
        return None;
    }

    serde_json::to_string(&spans).ok()
}

pub fn apply_bold(buffer: &TextBuffer) -> bool {
    toggle_named_tag(buffer, TAG_BOLD)
}

pub fn apply_underline(buffer: &TextBuffer) -> bool {
    toggle_named_tag(buffer, TAG_UNDERLINE)
}

pub fn apply_color(buffer: &TextBuffer, color_name: &str) -> bool {
    let Some((tag_name, _)) = COLOR_TAGS.iter().find(|(_, color)| *color == color_name) else {
        return false;
    };

    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    remove_color_tags(buffer, &start, &end);
    buffer.apply_tag_by_name(tag_name, &start, &end);
    true
}

pub fn clear_formatting(buffer: &TextBuffer) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    buffer.remove_tag_by_name(TAG_BOLD, &start, &end);
    buffer.remove_tag_by_name(TAG_UNDERLINE, &start, &end);
    remove_color_tags(buffer, &start, &end);
    true
}

fn toggle_named_tag(buffer: &TextBuffer, tag_name: &str) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };

    if selection_has_tag(buffer, tag_name, &start, &end) {
        buffer.remove_tag_by_name(tag_name, &start, &end);
    } else {
        buffer.apply_tag_by_name(tag_name, &start, &end);
    }

    true
}

fn remove_color_tags(buffer: &TextBuffer, start: &gtk::TextIter, end: &gtk::TextIter) {
    for (tag_name, _) in COLOR_TAGS {
        buffer.remove_tag_by_name(tag_name, start, end);
    }
}

fn selection_has_tag(
    buffer: &TextBuffer,
    tag_name: &str,
    start: &gtk::TextIter,
    end: &gtk::TextIter,
) -> bool {
    let Some(tag) = buffer.tag_table().lookup(tag_name) else {
        return false;
    };

    let mut iter = start.clone();
    while !iter.equal(end) {
        if !iter.has_tag(&tag) {
            return false;
        }

        if !iter.forward_char() {
            break;
        }
    }

    true
}

fn insert_spans(buffer: &TextBuffer, spans: &[RichTextSpan]) {
    for span in spans {
        let start = buffer.end_iter();
        let mut end = start.clone();
        buffer.insert(&mut end, &span.text);

        if span.bold {
            buffer.apply_tag_by_name(TAG_BOLD, &start, &end);
        }

        if span.underline {
            buffer.apply_tag_by_name(TAG_UNDERLINE, &start, &end);
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

    while !iter.equal(&end) {
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
                underline: current_style.underline,
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
            underline: current_style.underline,
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

    if let Some(tag) = buffer.tag_table().lookup(TAG_UNDERLINE) {
        style.underline = iter.has_tag(&tag);
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
