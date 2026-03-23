use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::domain::note::{NewNote, NoteDocument, NoteId, NoteSummary};
use orbital_core::{NoteRepository, OrbitalApp, OrbitalDatabase, OrbitalPaths};

mod block_layout;
mod rich_text;
mod settings;

const AUTOSAVE_DELAY_MS: u64 = 900;
const BLOCK_CHROME_TOP: i32 = 28;
const BLOCK_CHROME_BOTTOM: i32 = 24;
const MAX_UNDO_STEPS: usize = 100;
const MAX_HISTORY_STEPS: usize = 100;
const SIDEBAR_EXPANDED_WIDTH: i32 = 320;
const SIDEBAR_COLLAPSED_WIDTH: i32 = 180;

fn main() {
    adw::init().expect("Failed to initialize Libadwaita");

    let app = adw::Application::builder()
        .application_id(OrbitalApp::Drift.application_id())
        .build();

    app.connect_activate(|app| match DriftUi::build(app) {
        Ok(window) => window.present(),
        Err(error) => {
            eprintln!("Failed to start Drift: {error}");

            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Drift")
                .default_width(720)
                .default_height(480)
                .build();

            let content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(12)
                .margin_top(24)
                .margin_bottom(24)
                .margin_start(24)
                .margin_end(24)
                .build();

            let title = gtk::Label::builder()
                .label("Drift could not start")
                .xalign(0.0)
                .build();
            title.add_css_class("title-2");

            let body = gtk::Label::builder()
                .label(error.to_string())
                .wrap(true)
                .xalign(0.0)
                .selectable(true)
                .build();

            content.append(&title);
            content.append(&body);
            window.set_content(Some(&content));
            window.present();
        }
    });

    app.run();
}

struct DriftUi {
    database: OrbitalDatabase,
    paths: OrbitalPaths,
    list_box: gtk::ListBox,
    title_entry: gtk::Entry,
    canvas_grid: gtk::DrawingArea,
    body_canvas_fixed: gtk::Fixed,
    text_block_preview_frame: gtk::Frame,
    status_label: gtk::Label,
    edit_revealer: gtk::Revealer,
    notes: RefCell<Vec<NoteSummary>>,
    selected_note_id: RefCell<Option<NoteId>>,
    autosave_source: RefCell<Option<glib::SourceId>>,
    settings: RefCell<settings::DriftSettings>,
    text_blocks: RefCell<Vec<Rc<TextBlockWidget>>>,
    active_block_id: RefCell<Option<String>>,
    preview_block_id: RefCell<Option<String>>,
    preview_layout: RefCell<Option<block_layout::TextBlockLayout>>,
    next_block_id: Cell<u32>,
    history_undo_stack: RefCell<Vec<DriftHistorySnapshot>>,
    history_redo_stack: RefCell<Vec<DriftHistorySnapshot>>,
    history_suspended: Cell<bool>,
    title_history_pending: Cell<bool>,
    sidebar_collapsed: Cell<bool>,
    bold_mode: Cell<bool>,
    italic_mode: Cell<bool>,
    underline_mode: Cell<bool>,
    strikethrough_mode: Cell<bool>,
    color_mode: RefCell<Option<String>>,
    dirty: Cell<bool>,
    loading_ui: Cell<bool>,
}

struct TextBlockWidget {
    id: String,
    frame: gtk::Fixed,
    editor_frame: gtk::Frame,
    drag_handle: gtk::Box,
    resize_handle: gtk::Frame,
    buffer: gtk::TextBuffer,
    view: gtk::TextView,
    layout: RefCell<block_layout::TextBlockLayout>,
    undo_stack: RefCell<Vec<EditorSnapshot>>,
    redo_stack: RefCell<Vec<EditorSnapshot>>,
    last_snapshot: RefCell<EditorSnapshot>,
    restoring_history: Cell<bool>,
    hovered: Cell<bool>,
    interacting: Cell<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorSnapshot {
    plain_text: String,
    markup: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DriftHistorySnapshot {
    notes: Vec<NoteDocument>,
    selected_note_id: Option<NoteId>,
}

impl DriftUi {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let app_settings = settings::DriftSettings::load(&paths);
        let initial_layout = block_layout::default_note_canvas_layout(app_settings.grid_size());
        let database = OrbitalDatabase::open(&paths)?;

        let new_button = gtk::Button::builder()
            .icon_name("document-new-symbolic")
            .tooltip_text("New Page")
            .build();
        let undo_button = gtk::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text("Undo")
            .build();
        let redo_button = gtk::Button::builder()
            .icon_name("edit-redo-symbolic")
            .tooltip_text("Redo")
            .build();
        let edit_button = gtk::ToggleButton::builder()
            .icon_name("document-edit-symbolic")
            .tooltip_text("Edit")
            .build();
        new_button.add_css_class("header-action");
        undo_button.add_css_class("header-action");
        redo_button.add_css_class("header-action");
        edit_button.add_css_class("header-action");

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list_box.add_css_class("boxed-list");

        let title_entry = gtk::Entry::builder()
            .placeholder_text("Page title")
            .hexpand(true)
            .build();

        let body_canvas_fixed = gtk::Fixed::new();
        body_canvas_fixed.set_size_request(block_layout::CANVAS_WIDTH, block_layout::CANVAS_HEIGHT);
        let canvas_grid = gtk::DrawingArea::new();
        canvas_grid.set_content_width(block_layout::CANVAS_WIDTH);
        canvas_grid.set_content_height(block_layout::CANVAS_HEIGHT);

        let text_block_preview_frame = gtk::Frame::new(None);
        let text_block_preview_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text_block_preview_frame.set_child(Some(&text_block_preview_body));
        let initial_text_block = initial_layout
            .blocks
            .first()
            .map(|block| block.layout())
            .unwrap_or(block_layout::TextBlockLayout {
                x: 0,
                y: 0,
                width: 320,
                height: 220,
            });
        text_block_preview_frame.set_size_request(
            initial_text_block.width,
            initial_text_block.height,
        );
        text_block_preview_frame.add_css_class("card");
        text_block_preview_frame.set_can_target(false);
        text_block_preview_frame.set_opacity(0.75);
        text_block_preview_frame.set_visible(false);

        let status_label = gtk::Label::builder()
            .label("Ready")
            .xalign(0.0)
            .build();
        status_label.add_css_class("dim-label");

        let edit_revealer = gtk::Revealer::builder()
            .reveal_child(false)
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .build();

        let ui = Rc::new(Self {
            database,
            paths,
            list_box,
            title_entry,
            canvas_grid,
            body_canvas_fixed,
            text_block_preview_frame,
            status_label,
            edit_revealer,
            notes: RefCell::new(Vec::new()),
            selected_note_id: RefCell::new(None),
            autosave_source: RefCell::new(None),
            settings: RefCell::new(app_settings),
            text_blocks: RefCell::new(Vec::new()),
            active_block_id: RefCell::new(initial_layout.active_block_id.clone()),
            preview_block_id: RefCell::new(None),
            preview_layout: RefCell::new(None),
            next_block_id: Cell::new(initial_layout.blocks.len() as u32 + 1),
            history_undo_stack: RefCell::new(Vec::new()),
            history_redo_stack: RefCell::new(Vec::new()),
            history_suspended: Cell::new(false),
            title_history_pending: Cell::new(true),
            sidebar_collapsed: Cell::new(true),
            bold_mode: Cell::new(false),
            italic_mode: Cell::new(false),
            underline_mode: Cell::new(false),
            strikethrough_mode: Cell::new(false),
            color_mode: RefCell::new(None),
            dirty: Cell::new(false),
            loading_ui: Cell::new(false),
        });

        let settings_button = gtk::Button::builder()
            .icon_name("preferences-system-symbolic")
            .tooltip_text("Settings")
            .build();

        let window = build_window(
            app,
            &ui,
            &new_button,
            &undo_button,
            &redo_button,
            &edit_button,
            &settings_button,
        );
        connect_actions(
            &ui,
            &new_button,
            &undo_button,
            &redo_button,
            &edit_button,
            &settings_button,
            &window,
        );
        ui.reload_notes(None)?;

        if ui.notes.borrow().is_empty() {
            ui.create_note(false)?;
        }

        Ok(window)
    }

    fn reload_notes(self: &Rc<Self>, preferred_note: Option<NoteId>) -> orbital_core::OrbitalResult<()> {
        let repository = self.repository();
        let notes = repository.list_active()?;
        let selection = preferred_note.or_else(|| self.selected_note_id.borrow().clone());

        self.loading_ui.set(true);
        self.notes.replace(notes.clone());
        self.clear_list_box();

        for note in &notes {
            self.list_box
                .append(&build_note_row(self, note, self.sidebar_collapsed.get()));
        }

        let selected_index = selection.as_ref().and_then(|target| {
            notes.iter()
                .position(|note| note.id == *target)
                .map(|index| index as i32)
        });

        if let Some(index) = selected_index {
            if let Some(row) = self.list_box.row_at_index(index) {
                self.list_box.select_row(Some(&row));
                self.load_note_into_editor(index as usize)?;
            }
        } else if let Some(first_row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&first_row));
            self.load_note_into_editor(0)?;
        } else {
            self.clear_editor();
        }

        self.loading_ui.set(false);
        Ok(())
    }

    fn create_note(self: &Rc<Self>, record_history: bool) -> orbital_core::OrbitalResult<()> {
        if record_history {
            self.record_history_checkpoint()?;
        }

        let repository = self.repository();
        let mut new_note = NewNote::new(generate_note_id(), "Untitled page", "");
        new_note.body_layout = block_layout::serialize_layout(&block_layout::default_note_canvas_layout(
            self.grid_size(),
        ));

        let note = repository.create(new_note)?;

        self.selected_note_id.replace(Some(note.summary.id.clone()));
        self.reload_notes(Some(note.summary.id.clone()))?;
        self.populate_editor(&note);
        self.set_status("New page created");

        Ok(())
    }

    fn assemble_current_note_document(&self) -> orbital_core::OrbitalResult<Option<NoteDocument>> {
        let Some(note_id) = self.selected_note_id.borrow().clone() else {
            return Ok(None);
        };

        let title = self.title_entry.text().trim().to_string();
        let layout = self.snapshot_layout();
        let body = block_layout::compose_note_body(&layout);
        let body_markup = if layout.blocks.len() == 1 {
            layout.blocks.first().and_then(|block| block.body_markup.clone())
        } else {
            None
        };

        let existing = self
            .repository()
            .get(&note_id)?
            .ok_or_else(|| orbital_core::OrbitalError::NotFound {
                entity: "note",
                id: note_id.to_string(),
            })?;

        Ok(Some(NoteDocument {
            summary: NoteSummary {
                title: if title.is_empty() {
                    "Untitled page".to_string()
                } else {
                    title
                },
                ..existing.summary
            },
            body,
            body_markup,
            body_layout: block_layout::serialize_layout(&layout),
        }))
    }

    fn refresh_note_summaries(self: &Rc<Self>, preferred_note: Option<NoteId>) -> orbital_core::OrbitalResult<()> {
        let repository = self.repository();
        let notes = repository.list_active()?;
        let selection = preferred_note.or_else(|| self.selected_note_id.borrow().clone());

        self.loading_ui.set(true);
        self.notes.replace(notes.clone());
        self.clear_list_box();

        for note in &notes {
            self.list_box
                .append(&build_note_row(self, note, self.sidebar_collapsed.get()));
        }

        if let Some(target) = selection {
            if let Some(index) = notes.iter().position(|note| note.id == target) {
                if let Some(row) = self.list_box.row_at_index(index as i32) {
                    self.list_box.select_row(Some(&row));
                }
            }
        }

        self.loading_ui.set(false);
        Ok(())
    }

    fn load_note_into_editor(self: &Rc<Self>, index: usize) -> orbital_core::OrbitalResult<()> {
        let Some(summary) = self.notes.borrow().get(index).cloned() else {
            return Ok(());
        };

        if let Some(current_id) = self.selected_note_id.borrow().clone() {
            if current_id != summary.id {
                let _ = self.flush_autosave()?;
            }
        }

        let document = self
            .repository()
            .get(&summary.id)?
            .ok_or_else(|| orbital_core::OrbitalError::NotFound {
                entity: "note",
                id: summary.id.to_string(),
            })?;

        self.selected_note_id
            .replace(Some(document.summary.id.clone()));
        self.populate_editor(&document);
        self.set_status("Page loaded");

        Ok(())
    }

    fn begin_page_rename(self: &Rc<Self>, note_id: &NoteId) -> orbital_core::OrbitalResult<()> {
        self.reload_notes(Some(note_id.clone()))?;
        self.title_entry.grab_focus();
        self.title_entry.select_region(0, -1);
        self.set_status("Rename page");
        Ok(())
    }

    fn duplicate_page(self: &Rc<Self>, note_id: &NoteId) -> orbital_core::OrbitalResult<()> {
        self.record_history_checkpoint()?;
        let duplicated = self.repository().duplicate(note_id, generate_note_id())?;
        self.reload_notes(Some(duplicated.summary.id.clone()))?;
        self.set_status("Page duplicated");
        Ok(())
    }

    fn remove_page(self: &Rc<Self>, note_id: &NoteId) -> orbital_core::OrbitalResult<()> {
        self.record_history_checkpoint()?;
        let notes = self.notes.borrow().clone();
        let fallback_selection = notes
            .iter()
            .position(|note| note.id == *note_id)
            .and_then(|index| {
                notes.get(index + 1)
                    .or_else(|| index.checked_sub(1).and_then(|previous| notes.get(previous)))
            })
            .map(|note| note.id.clone());

        self.repository().archive(note_id)?;
        self.reload_notes(fallback_selection)?;
        self.set_status("Page removed");
        Ok(())
    }

    fn reorder_page(
        self: &Rc<Self>,
        source_id: &NoteId,
        target_id: &NoteId,
        place_after: bool,
    ) -> orbital_core::OrbitalResult<()> {
        if source_id == target_id {
            return Ok(());
        }

        self.record_history_checkpoint()?;
        self.repository().reorder(source_id, target_id, place_after)?;
        self.set_status("Page order updated");
        let ui = Rc::clone(self);
        let selected_note = source_id.clone();
        glib::idle_add_local_once(move || {
            if let Err(error) = ui.reload_notes(Some(selected_note)) {
                ui.set_status(&format!("Refresh failed: {error}"));
            }
        });
        Ok(())
    }

    fn populate_editor(self: &Rc<Self>, note: &NoteDocument) {
        self.cancel_autosave();
        self.loading_ui.set(true);
        self.dirty.set(false);
        self.title_history_pending.set(true);
        self.preview_layout.borrow_mut().take();
        self.preview_block_id.borrow_mut().take();
        self.text_block_preview_frame.set_visible(false);
        self.title_entry.set_text(&note.summary.title);
        self.clear_canvas_blocks();

        let layout = block_layout::deserialize_layout(
            note.body_layout.as_deref(),
            self.grid_size(),
            &note.body,
            note.body_markup.as_deref(),
        );

        self.next_block_id
            .set(layout.blocks.len().max(1) as u32 + 1);

        for block in &layout.blocks {
            let widget = build_text_block_widget(self, block);
            self.body_canvas_fixed.put(&widget.frame, 0.0, 0.0);
            self.text_blocks.borrow_mut().push(widget);
        }

        let active_id = layout
            .active_block_id
            .or_else(|| layout.blocks.first().map(|block| block.id.clone()));
        self.active_block_id.replace(active_id);
        self.apply_canvas_layout();
        self.sync_all_block_chrome();
        self.loading_ui.set(false);
    }

    fn clear_editor(self: &Rc<Self>) {
        self.cancel_autosave();
        self.loading_ui.set(true);
        self.dirty.set(false);
        self.title_history_pending.set(true);
        self.selected_note_id.replace(None);
        self.preview_layout.borrow_mut().take();
        self.preview_block_id.borrow_mut().take();
        self.text_block_preview_frame.set_visible(false);
        self.title_entry.set_text("");
        self.clear_canvas_blocks();

        let layout = block_layout::default_note_canvas_layout(self.grid_size());
        self.next_block_id
            .set(layout.blocks.len().max(1) as u32 + 1);

        for block in &layout.blocks {
            let widget = build_text_block_widget(self, block);
            self.body_canvas_fixed.put(&widget.frame, 0.0, 0.0);
            self.text_blocks.borrow_mut().push(widget);
        }

        self.active_block_id.replace(layout.active_block_id);
        self.apply_canvas_layout();
        self.sync_all_block_chrome();
        self.loading_ui.set(false);
    }

    fn clear_list_box(&self) {
        let mut children = Vec::new();
        let mut current = self.list_box.first_child();

        while let Some(child) = current {
            let next = child.next_sibling();
            children.push(child);
            current = next;
        }

        for child in children {
            if child.parent().as_ref() == Some(self.list_box.upcast_ref()) {
                self.list_box.remove(&child);
            }
        }
    }

    fn persist_editor_to_database(&self) -> orbital_core::OrbitalResult<Option<NoteDocument>> {
        let Some(note) = self.assemble_current_note_document()? else {
            return Ok(None);
        };

        let saved = self.repository().save(&note)?;
        self.dirty.set(false);
        Ok(Some(saved))
    }

    fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
    }

    fn apply_canvas_layout(&self) {
        for widget in self.text_blocks.borrow().iter() {
            let layout = widget.layout.borrow().clone().clamp_to_canvas(self.grid_size());
            self.render_block_layout(widget, &layout);
        }
    }

    fn render_block_layout(&self, widget: &TextBlockWidget, layout: &block_layout::TextBlockLayout) {
        widget
            .frame
            .set_size_request(layout.width, layout.height + BLOCK_CHROME_TOP + BLOCK_CHROME_BOTTOM);
        widget.editor_frame.set_size_request(layout.width, layout.height);
        widget.drag_handle.set_size_request(layout.width, BLOCK_CHROME_TOP);
        widget
            .frame
            .move_(&widget.drag_handle, 0.0, 0.0);
        widget
            .frame
            .move_(&widget.editor_frame, 0.0, BLOCK_CHROME_TOP as f64);
        widget.frame.move_(
            &widget.resize_handle,
            0.0,
            (BLOCK_CHROME_TOP + layout.height) as f64,
        );
        self.body_canvas_fixed.move_(
            &widget.frame,
            layout.x as f64,
            (layout.y - BLOCK_CHROME_TOP) as f64,
        );
    }

    fn render_preview_layout(&self, layout: &block_layout::TextBlockLayout) {
        self.text_block_preview_frame
            .set_size_request(layout.width, layout.height);
        self.body_canvas_fixed.move_(
            &self.text_block_preview_frame,
            layout.x as f64,
            layout.y as f64,
        );
    }

    fn update_text_block_layout(&self, block_id: &str, new_layout: block_layout::TextBlockLayout) {
        let preview = new_layout
            .preview_constrained(self.grid_size())
            .clamp_to_canvas(self.grid_size());
        self.preview_block_id
            .replace(Some(block_id.to_string()));
        self.preview_layout.replace(Some(preview.clone()));
        self.render_preview_layout(&preview);
    }

    fn finalize_text_block_layout(&self) {
        let Some(block_id) = self.preview_block_id.borrow().clone() else {
            return;
        };

        let finalized = self
            .preview_layout
            .borrow()
            .clone()
            .or_else(|| self.current_text_block_layout(&block_id))
            .unwrap_or(block_layout::TextBlockLayout {
                x: 0,
                y: 0,
                width: self.grid_size() * 44,
                height: self.grid_size() * 28,
            })
            .snapped_to_grid(self.grid_size())
            .clamp_to_canvas(self.grid_size());

        if let Some(widget) = self.find_text_block(&block_id) {
            widget.layout.replace(finalized.clone());
            self.render_block_layout(&widget, &finalized);
        }
    }

    fn begin_text_block_interaction(&self, block_id: &str) {
        let Some(widget) = self.find_text_block(block_id) else {
            return;
        };

        widget.interacting.set(true);
        let current_layout = widget
            .layout
            .borrow()
            .clone()
            .clamp_to_canvas(self.grid_size());
        self.preview_block_id
            .replace(Some(block_id.to_string()));
        self.preview_layout.replace(Some(current_layout.clone()));
        self.render_preview_layout(&current_layout);
        self.text_block_preview_frame.set_visible(true);
        widget.frame.set_opacity(0.28);
        self.sync_block_chrome(&widget);
    }

    fn finish_text_block_interaction(&self) {
        if let Some(block_id) = self.preview_block_id.borrow().clone() {
            if let Some(widget) = self.find_text_block(&block_id) {
                widget.frame.set_opacity(1.0);
                widget.interacting.set(false);
                self.sync_block_chrome(&widget);
            }
        }

        self.preview_layout.borrow_mut().take();
        self.preview_block_id.borrow_mut().take();
        self.text_block_preview_frame.set_visible(false);
    }

    fn pending_format(&self) -> rich_text::PendingFormat {
        rich_text::PendingFormat {
            bold: self.bold_mode.get(),
            italic: self.italic_mode.get(),
            underline: self.underline_mode.get(),
            strikethrough: self.strikethrough_mode.get(),
            heading_1: false,
            color: self.color_mode.borrow().clone(),
        }
    }

    fn mark_dirty(&self) {
        if self.loading_ui.get() {
            return;
        }

        self.dirty.set(true);
        self.set_status("Editing...");
    }

    fn sync_block_chrome(&self, widget: &TextBlockWidget) {
        let visible = widget.hovered.get() || widget.interacting.get();
        let opacity = if visible { 1.0 } else { 0.0 };

        widget.drag_handle.set_opacity(opacity);
        widget.resize_handle.set_opacity(opacity);
    }

    fn sync_all_block_chrome(&self) {
        for widget in self.text_blocks.borrow().iter() {
            self.sync_block_chrome(widget);
        }
    }

    fn grid_size(&self) -> i32 {
        self.settings.borrow().grid_size()
    }

    fn update_grid_density(&self, grid_density: settings::GridDensity) {
        self.settings.borrow_mut().grid_density = grid_density;

        if let Err(error) = self.settings.borrow().save(&self.paths) {
            self.set_status(&format!("Settings save failed: {error}"));
            return;
        }

        self.canvas_grid.queue_draw();
        self.apply_canvas_layout();
        self.set_status("Grid setting updated");
    }

    fn current_text_block_layout(&self, block_id: &str) -> Option<block_layout::TextBlockLayout> {
        if self.preview_block_id.borrow().as_deref() == Some(block_id) {
            if let Some(layout) = self.preview_layout.borrow().clone() {
                return Some(layout.clamp_to_canvas(self.grid_size()));
            }
        }

        self.find_text_block(block_id)
            .map(|widget| widget.layout.borrow().clone().clamp_to_canvas(self.grid_size()))
    }

    fn point_hits_any_block(&self, x: f64, y: f64) -> bool {
        let x = x.round() as i32;
        let y = y.round() as i32;

        self.text_blocks.borrow().iter().rev().any(|widget| {
            let layout = self
                .current_text_block_layout(&widget.id)
                .unwrap_or_else(|| widget.layout.borrow().clone());

            x >= layout.x
                && x <= layout.x + layout.width
                && y >= layout.y - BLOCK_CHROME_TOP
                && y <= layout.y + layout.height + BLOCK_CHROME_BOTTOM
        })
    }

    fn create_text_block_at(self: &Rc<Self>, x: i32, y: i32) -> orbital_core::OrbitalResult<()> {
        self.record_history_checkpoint()?;

        let grid_size = self.grid_size();
        let block = block_layout::TextBlockState {
            id: self.next_text_block_id(),
            x,
            y,
            width: grid_size * 44,
            height: grid_size * 28,
            body: String::new(),
            body_markup: None,
        }
        .with_layout(
            block_layout::TextBlockLayout {
                x,
                y,
                width: grid_size * 44,
                height: grid_size * 28,
            }
            .snapped_to_grid(grid_size)
            .clamp_to_canvas(grid_size),
        );

        self.preview_layout.borrow_mut().take();
        self.preview_block_id.borrow_mut().take();
        self.text_block_preview_frame.set_visible(false);
        let widget = build_text_block_widget(self, &block);
        self.body_canvas_fixed.put(&widget.frame, 0.0, 0.0);
        self.text_blocks.borrow_mut().push(widget.clone());
        self.active_block_id.replace(Some(block.id.clone()));
        self.render_block_layout(&widget, &block.layout());
        self.sync_block_chrome(&widget);
        widget.view.grab_focus();
        self.mark_dirty();
        self.save_immediately("Text block created")
    }

    fn next_text_block_id(&self) -> String {
        let next = self.next_block_id.get();
        self.next_block_id.set(next + 1);
        format!("text-block-{next}")
    }

    fn find_text_block(&self, block_id: &str) -> Option<Rc<TextBlockWidget>> {
        self.text_blocks
            .borrow()
            .iter()
            .find(|widget| widget.id == block_id)
            .cloned()
    }

    fn set_active_block(&self, block_id: Option<String>) {
        self.active_block_id.replace(block_id);
    }

    fn active_text_block(&self) -> Option<Rc<TextBlockWidget>> {
        let block_id = self.active_block_id.borrow().clone()?;
        self.find_text_block(&block_id)
    }

    fn active_buffer(&self) -> Option<gtk::TextBuffer> {
        self.active_text_block().map(|widget| widget.buffer.clone())
    }

    fn apply_paragraph_style(
        self: &Rc<Self>,
        style: rich_text::ParagraphStyle,
        success_message: &str,
    ) -> orbital_core::OrbitalResult<bool> {
        let Some(buffer) = self.active_buffer() else {
            return Ok(false);
        };

        self.record_history_checkpoint()?;
        let changed = rich_text::set_paragraph_style(&buffer, style);
        if changed {
            self.mark_dirty();
            self.save_immediately(success_message)?;
        }

        Ok(changed)
    }

    fn apply_checklist(self: &Rc<Self>) -> orbital_core::OrbitalResult<bool> {
        let Some(buffer) = self.active_buffer() else {
            return Ok(false);
        };

        self.record_history_checkpoint()?;
        let changed = insert_checklist(&buffer);
        if changed {
            self.mark_dirty();
            self.save_immediately("Checklist saved")?;
        }

        Ok(changed)
    }

    fn handle_undo(self: &Rc<Self>) -> orbital_core::OrbitalResult<bool> {
        let Some(snapshot) = self.history_undo_stack.borrow_mut().pop() else {
            self.set_status("Nothing to undo");
            return Ok(false);
        };

        let current = self.capture_history_snapshot()?;
        Self::push_history_snapshot(&self.history_redo_stack, current);
        self.restore_history_snapshot(&snapshot, "Undo applied")?;
        Ok(true)
    }

    fn handle_redo(self: &Rc<Self>) -> orbital_core::OrbitalResult<bool> {
        let Some(snapshot) = self.history_redo_stack.borrow_mut().pop() else {
            self.set_status("Nothing to redo");
            return Ok(false);
        };

        let current = self.capture_history_snapshot()?;
        Self::push_history_snapshot(&self.history_undo_stack, current);
        self.restore_history_snapshot(&snapshot, "Redo applied")?;
        Ok(true)
    }

    fn handle_shortcut(
        self: &Rc<Self>,
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) -> orbital_core::OrbitalResult<bool> {
        let control = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);

        if !control {
            return Ok(false);
        }

        match keyval.to_unicode().map(|ch| ch.to_ascii_lowercase()) {
            Some('z') if shift => self.handle_redo(),
            Some('z') => self.handle_undo(),
            Some('y') => self.handle_redo(),
            Some('b') => self.apply_inline_shortcut(|buffer| rich_text::set_bold(&buffer, true)),
            Some('i') => self.apply_inline_shortcut(|buffer| rich_text::set_italic(&buffer, true)),
            Some('u') => self.apply_inline_shortcut(|buffer| rich_text::set_underline(&buffer, true)),
            Some('1') => self.apply_paragraph_style(rich_text::ParagraphStyle::Heading1, "Heading saved"),
            Some('0') => self.apply_paragraph_style(rich_text::ParagraphStyle::Normal, "Paragraph saved"),
            Some('7') => {
                let Some(buffer) = self.active_buffer() else {
                    return Ok(false);
                };
                let changed = if shift {
                    insert_checklist(&buffer)
                } else {
                    insert_bullet_list(&buffer)
                };
                if changed {
                    self.mark_dirty();
                    self.save_immediately("List saved")?;
                }
                Ok(changed)
            }
            _ => Ok(false),
        }
    }

    fn apply_inline_shortcut<F>(self: &Rc<Self>, action: F) -> orbital_core::OrbitalResult<bool>
    where
        F: FnOnce(gtk::TextBuffer) -> bool,
    {
        let Some(buffer) = self.active_buffer() else {
            return Ok(false);
        };

        self.record_history_checkpoint()?;
        let changed = action(buffer);
        if changed {
            self.mark_dirty();
            self.save_immediately("Formatting saved")?;
        }

        Ok(changed)
    }

    fn snapshot_layout(&self) -> block_layout::NoteCanvasLayout {
        let blocks = self
            .text_blocks
            .borrow()
            .iter()
            .map(|widget| block_layout::TextBlockState {
                id: widget.id.clone(),
                x: widget.layout.borrow().x,
                y: widget.layout.borrow().y,
                width: widget.layout.borrow().width,
                height: widget.layout.borrow().height,
                body: widget
                    .buffer
                    .text(&widget.buffer.start_iter(), &widget.buffer.end_iter(), true)
                    .to_string(),
                body_markup: rich_text::serialize_buffer(&widget.buffer),
            })
            .collect();

        block_layout::NoteCanvasLayout {
            blocks,
            active_block_id: self.active_block_id.borrow().clone(),
        }
    }

    fn capture_history_snapshot(&self) -> orbital_core::OrbitalResult<DriftHistorySnapshot> {
        let repository = self.repository();
        let mut notes = Vec::new();
        let active_notes = repository.list_active()?;
        let current_note = self.assemble_current_note_document()?;

        for summary in active_notes {
            if current_note
                .as_ref()
                .map(|note| note.summary.id == summary.id)
                .unwrap_or(false)
            {
                if let Some(note) = current_note.clone() {
                    notes.push(note);
                }
            } else if let Some(note) = repository.get(&summary.id)? {
                notes.push(note);
            }
        }

        Ok(DriftHistorySnapshot {
            notes,
            selected_note_id: self.selected_note_id.borrow().clone(),
        })
    }

    fn push_history_snapshot(
        stack: &RefCell<Vec<DriftHistorySnapshot>>,
        snapshot: DriftHistorySnapshot,
    ) {
        let mut stack = stack.borrow_mut();
        if stack.last().map(|existing| existing == &snapshot).unwrap_or(false) {
            return;
        }

        stack.push(snapshot);
        if stack.len() > MAX_HISTORY_STEPS {
            stack.remove(0);
        }
    }

    fn record_history_checkpoint(&self) -> orbital_core::OrbitalResult<()> {
        if self.loading_ui.get() || self.history_suspended.get() {
            return Ok(());
        }

        let snapshot = self.capture_history_snapshot()?;
        Self::push_history_snapshot(&self.history_undo_stack, snapshot);
        self.history_redo_stack.borrow_mut().clear();
        Ok(())
    }

    fn restore_history_snapshot(
        self: &Rc<Self>,
        snapshot: &DriftHistorySnapshot,
        success_message: &str,
    ) -> orbital_core::OrbitalResult<()> {
        self.cancel_autosave();
        self.history_suspended.set(true);

        let result = (|| -> orbital_core::OrbitalResult<()> {
            let repository = self.repository();
            let target_ids: HashSet<String> = snapshot
                .notes
                .iter()
                .map(|note| note.summary.id.to_string())
                .collect();

            for note in &snapshot.notes {
                if repository.get(&note.summary.id)?.is_some() {
                    repository.save(note)?;
                } else {
                    let mut new_note = NewNote::new(
                        note.summary.id.clone(),
                        note.summary.title.clone(),
                        note.body.clone(),
                    );
                    new_note.body_markup = note.body_markup.clone();
                    new_note.body_layout = note.body_layout.clone();
                    new_note.display_order = Some(note.summary.display_order);
                    new_note.tags = note.summary.tags.clone();
                    repository.create(new_note)?;
                }
            }

            for current in repository.list_active()? {
                if !target_ids.contains(current.id.as_str()) {
                    repository.archive(&current.id)?;
                }
            }

            self.reload_notes(snapshot.selected_note_id.clone())?;
            if self.notes.borrow().is_empty() {
                self.clear_editor();
            }

            self.dirty.set(false);
            self.title_history_pending.set(true);
            self.set_status(success_message);
            Ok(())
        })();

        self.history_suspended.set(false);
        result
    }

    fn clear_canvas_blocks(&self) {
        let widgets = self.text_blocks.borrow().clone();
        for widget in widgets {
            self.body_canvas_fixed.remove(&widget.frame);
        }

        self.text_blocks.borrow_mut().clear();
    }

    fn schedule_autosave(self: &Rc<Self>) {
        if self.loading_ui.get() || self.selected_note_id.borrow().is_none() {
            return;
        }

        self.cancel_autosave();
        self.set_status("Autosave scheduled");

        let ui = Rc::clone(self);
        let source_id = glib::timeout_add_local(
            std::time::Duration::from_millis(AUTOSAVE_DELAY_MS),
            move || {
                if let Err(error) = ui.flush_autosave() {
                    ui.set_status(&format!("Autosave failed: {error}"));
                }

                glib::ControlFlow::Break
            },
        );

        self.autosave_source.replace(Some(source_id));
    }

    fn cancel_autosave(&self) {
        if let Some(source_id) = self.autosave_source.borrow_mut().take() {
            source_id.remove();
        }
    }

    fn flush_autosave(self: &Rc<Self>) -> orbital_core::OrbitalResult<Option<NoteDocument>> {
        self.cancel_autosave();

        if !self.dirty.get() {
            return Ok(None);
        }

        let saved = self.persist_editor_to_database()?;

        if let Some(saved) = saved {
            self.selected_note_id.replace(Some(saved.summary.id.clone()));
            self.refresh_note_summaries(Some(saved.summary.id.clone()))?;
            self.set_status("Autosaved");
            Ok(Some(saved))
        } else {
            Ok(None)
        }
    }

    fn save_immediately(self: &Rc<Self>, success_message: &str) -> orbital_core::OrbitalResult<()> {
        self.cancel_autosave();

        let Some(saved) = self.persist_editor_to_database()? else {
            return Ok(());
        };

        self.selected_note_id.replace(Some(saved.summary.id.clone()));
        self.refresh_note_summaries(Some(saved.summary.id.clone()))?;
        self.set_status(success_message);

        Ok(())
    }

    fn repository(&self) -> NoteRepository<'_> {
        NoteRepository::new(self.database.connection())
    }
}

fn build_text_block_widget(
    ui: &Rc<DriftUi>,
    block: &block_layout::TextBlockState,
) -> Rc<TextBlockWidget> {
    let buffer = rich_text::create_buffer();
    rich_text::set_buffer_content(&buffer, &block.body, block.body_markup.as_deref());

    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .monospace(false)
        .vexpand(true)
        .build();

    let drag_handle = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(4)
        .margin_end(4)
        .build();
    drag_handle.set_halign(gtk::Align::Fill);

    let block_title = gtk::Label::builder()
        .label("Text block")
        .xalign(0.0)
        .build();
    block_title.add_css_class("heading");
    let block_hint = gtk::Label::builder()
        .label("Drag")
        .build();
    block_hint.add_css_class("dim-label");

    let block_title_chip = gtk::Frame::new(None);
    block_title_chip.set_child(Some(&block_title));
    block_title_chip.add_css_class("card");
    block_title_chip.set_margin_start(2);
    block_title_chip.set_margin_top(2);
    block_title_chip.set_margin_bottom(2);

    let block_hint_chip = gtk::Frame::new(None);
    block_hint_chip.set_child(Some(&block_hint));
    block_hint_chip.add_css_class("card");
    block_hint_chip.set_margin_end(2);
    block_hint_chip.set_margin_top(2);
    block_hint_chip.set_margin_bottom(2);

    let drag_spacer = gtk::Box::builder().hexpand(true).build();
    drag_handle.append(&block_title_chip);
    drag_handle.append(&drag_spacer);
    drag_handle.append(&block_hint_chip);
    drag_handle.set_opacity(0.0);

    let resize_label = gtk::Label::builder()
        .label("Resize")
        .halign(gtk::Align::End)
        .build();
    resize_label.add_css_class("dim-label");

    let resize_handle = gtk::Frame::new(None);
    resize_handle.set_child(Some(&resize_label));
    resize_handle.add_css_class("card");
    resize_handle.set_halign(gtk::Align::End);
    resize_handle.set_margin_end(6);
    resize_handle.set_margin_top(2);
    resize_handle.set_margin_bottom(2);
    resize_handle.set_opacity(0.0);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();
    let editor_frame = gtk::Frame::new(None);
    editor_frame.set_child(Some(&scroller));
    editor_frame.set_size_request(block.width, block.height);
    editor_frame.add_css_class("card");

    let frame = gtk::Fixed::new();
    frame.set_size_request(block.width, block.height + BLOCK_CHROME_TOP + BLOCK_CHROME_BOTTOM);
    frame.put(&drag_handle, 0.0, 0.0);
    frame.put(&editor_frame, 0.0, BLOCK_CHROME_TOP as f64);
    frame.put(
        &resize_handle,
        0.0,
        (BLOCK_CHROME_TOP + block.height) as f64,
    );

    let initial_snapshot = capture_snapshot(&buffer);

    let widget = Rc::new(TextBlockWidget {
        id: block.id.clone(),
        frame,
        editor_frame,
        drag_handle,
        resize_handle,
        buffer,
        view,
        layout: RefCell::new(block.layout()),
        undo_stack: RefCell::new(Vec::new()),
        redo_stack: RefCell::new(Vec::new()),
        last_snapshot: RefCell::new(initial_snapshot),
        restoring_history: Cell::new(false),
        hovered: Cell::new(false),
        interacting: Cell::new(false),
    });

    {
        let ui = Rc::clone(ui);
        let widget_for_focus = Rc::clone(&widget);
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(move |_| {
            ui.set_active_block(Some(widget_for_focus.id.clone()));
        });
        widget.view.add_controller(focus);
    }

    {
        let ui = Rc::clone(ui);
        let widget_for_insert = Rc::clone(&widget);
        widget.buffer.connect_insert_text(move |buffer, location, text| {
            if ui.loading_ui.get() {
                return;
            }

            ui.set_active_block(Some(widget_for_insert.id.clone()));
            let pending = ui.pending_format();
            if pending.is_plain() {
                return;
            }

            let start_offset = location.offset();
            let char_count = text.chars().count() as i32;
            let buffer = buffer.clone();
            let insert_mark = buffer.get_insert();

            glib::idle_add_local_once(move || {
                let end_offset = buffer.iter_at_mark(&insert_mark).offset();
                let applied_count = (end_offset - start_offset).max(0).min(char_count);

                rich_text::apply_pending_format_by_offsets(
                    &buffer,
                    start_offset,
                    applied_count,
                    &pending,
                );
            });
        });
    }

    {
        let ui = Rc::clone(ui);
        let widget_for_change = Rc::clone(&widget);
        widget.buffer.connect_changed(move |_| {
            if widget_for_change.restoring_history.get() {
                return;
            }

            ui.set_active_block(Some(widget_for_change.id.clone()));
            ui.mark_dirty();
            ui.schedule_autosave();
        });
    }

    {
        let widget_for_history = Rc::clone(&widget);
        let ui_for_history = Rc::clone(ui);
        widget.buffer.connect_begin_user_action(move |_| {
            if widget_for_history.restoring_history.get() {
                return;
            }

            if let Err(error) = ui_for_history.record_history_checkpoint() {
                ui_for_history.set_status(&format!("History checkpoint failed: {error}"));
                return;
            }

            let snapshot = widget_for_history.last_snapshot.borrow().clone();
            let mut undo_stack = widget_for_history.undo_stack.borrow_mut();
            undo_stack.push(snapshot);
            if undo_stack.len() > MAX_UNDO_STEPS {
                undo_stack.remove(0);
            }
            widget_for_history.redo_stack.borrow_mut().clear();
        });
    }

    {
        let widget_for_history = Rc::clone(&widget);
        widget.buffer.connect_end_user_action(move |buffer| {
            if widget_for_history.restoring_history.get() {
                return;
            }

            widget_for_history
                .last_snapshot
                .replace(capture_snapshot(buffer));
        });
    }

    {
        let ui = Rc::clone(ui);
        let widget_for_key = Rc::clone(&widget);
        let key = gtk::EventControllerKey::new();
        key.connect_key_pressed(move |_, keyval, _, state| {
            ui.set_active_block(Some(widget_for_key.id.clone()));

            if keyval == gtk::gdk::Key::Return
                && !state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && continue_list_or_checklist(&widget_for_key.buffer)
            {
                ui.mark_dirty();
                ui.schedule_autosave();
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });
        widget.view.add_controller(key);
    }

    {
        let ui_for_enter = Rc::clone(&ui);
        let widget_for_enter = Rc::clone(&widget);
        let hover = gtk::EventControllerMotion::new();
        hover.connect_enter(move |_, _, _| {
            widget_for_enter.hovered.set(true);
            ui_for_enter.sync_block_chrome(&widget_for_enter);
        });

        let ui_for_leave = Rc::clone(&ui);
        let widget_for_leave = Rc::clone(&widget);
        hover.connect_leave(move |_| {
            widget_for_leave.hovered.set(false);
            ui_for_leave.sync_block_chrome(&widget_for_leave);
        });
        widget.frame.add_controller(hover);
    }

    {
        let ui = Rc::clone(ui);
        let widget_for_drag = Rc::clone(&widget);
        let gesture = gtk::GestureDrag::new();
        let drag_origin = Rc::new(RefCell::new(None::<block_layout::TextBlockLayout>));

        {
            let ui = Rc::clone(&ui);
            let drag_origin = Rc::clone(&drag_origin);
            let widget_for_drag_begin = Rc::clone(&widget_for_drag);
            gesture.connect_drag_begin(move |_, _, _| {
                if let Err(error) = ui.record_history_checkpoint() {
                    ui.set_status(&format!("History checkpoint failed: {error}"));
                    return;
                }

                ui.set_active_block(Some(widget_for_drag_begin.id.clone()));
                ui.begin_text_block_interaction(&widget_for_drag_begin.id);
                drag_origin.replace(Some(widget_for_drag_begin.layout.borrow().clone()));
            });
        }

        {
            let ui = Rc::clone(&ui);
            let drag_origin = Rc::clone(&drag_origin);
            let widget_for_drag_update = Rc::clone(&widget_for_drag);
            gesture.connect_drag_update(move |_, offset_x, offset_y| {
                let Some(origin) = drag_origin.borrow().clone() else {
                    return;
                };

                ui.update_text_block_layout(
                    &widget_for_drag_update.id,
                    block_layout::TextBlockLayout {
                        x: origin.x + offset_x.round() as i32,
                        y: origin.y + offset_y.round() as i32,
                        width: origin.width,
                        height: origin.height,
                    },
                );
            });
        }

        {
            let ui = Rc::clone(&ui);
            gesture.connect_drag_end(move |_, _, _| {
                ui.finalize_text_block_layout();
                ui.finish_text_block_interaction();
                ui.mark_dirty();
                if let Err(error) = ui.save_immediately("Block position saved") {
                    ui.set_status(&format!("Layout save failed: {error}"));
                }
            });
        }

        widget.drag_handle.add_controller(gesture);
    }

    {
        let ui = Rc::clone(ui);
        let widget_for_resize = Rc::clone(&widget);
        let gesture = gtk::GestureDrag::new();
        let resize_origin = Rc::new(RefCell::new(None::<block_layout::TextBlockLayout>));

        {
            let ui = Rc::clone(&ui);
            let resize_origin = Rc::clone(&resize_origin);
            let widget_for_resize_begin = Rc::clone(&widget_for_resize);
            gesture.connect_drag_begin(move |_, _, _| {
                if let Err(error) = ui.record_history_checkpoint() {
                    ui.set_status(&format!("History checkpoint failed: {error}"));
                    return;
                }

                ui.set_active_block(Some(widget_for_resize_begin.id.clone()));
                ui.begin_text_block_interaction(&widget_for_resize_begin.id);
                resize_origin.replace(Some(widget_for_resize_begin.layout.borrow().clone()));
            });
        }

        {
            let ui = Rc::clone(&ui);
            let resize_origin = Rc::clone(&resize_origin);
            let widget_for_resize_update = Rc::clone(&widget_for_resize);
            gesture.connect_drag_update(move |_, offset_x, offset_y| {
                let Some(origin) = resize_origin.borrow().clone() else {
                    return;
                };

                ui.update_text_block_layout(
                    &widget_for_resize_update.id,
                    block_layout::TextBlockLayout {
                        x: origin.x,
                        y: origin.y,
                        width: origin.width + offset_x.round() as i32,
                        height: origin.height + offset_y.round() as i32,
                    },
                );
            });
        }

        {
            let ui = Rc::clone(&ui);
            gesture.connect_drag_end(move |_, _, _| {
                ui.finalize_text_block_layout();
                ui.finish_text_block_interaction();
                ui.mark_dirty();
                if let Err(error) = ui.save_immediately("Block size saved") {
                    ui.set_status(&format!("Layout save failed: {error}"));
                }
            });
        }

        widget.resize_handle.add_controller(gesture);
    }

    widget
}

fn build_window(
    app: &adw::Application,
    ui: &Rc<DriftUi>,
    new_button: &gtk::Button,
    undo_button: &gtk::Button,
    redo_button: &gtk::Button,
    edit_button: &gtk::ToggleButton,
    settings_button: &gtk::Button,
) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Drift")
        .default_width(1320)
        .default_height(860)
        .build();
    install_app_styles();

    let header_bar = adw::HeaderBar::new();
    let header_title = gtk::Label::builder()
        .label("Drift")
        .xalign(0.0)
        .build();
    header_title.add_css_class("title-3");

    let header_logo = gtk::Image::from_file(drift_logo_path());
    header_logo.set_pixel_size(42);
    header_logo.set_size_request(42, 42);

    let brand_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(4)
        .margin_start(8)
        .margin_end(12)
        .build();
    brand_box.append(&header_logo);
    brand_box.append(&header_title);

    let brand_frame = gtk::Frame::new(None);
    brand_frame.set_child(Some(&brand_box));
    brand_frame.add_css_class("card");
    brand_frame.set_margin_end(14);

    header_bar.pack_start(&brand_frame);
    header_bar.pack_start(new_button);
    header_bar.pack_start(undo_button);
    header_bar.pack_start(redo_button);
    header_bar.pack_start(edit_button);
    header_bar.pack_end(settings_button);
    header_bar.set_title_widget(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    let formatting_toolbar = build_formatting_toolbar(ui);
    ui.edit_revealer.set_child(Some(&formatting_toolbar));

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .build();

    let sidebar = build_sidebar(ui);
    let sidebar_separator = gtk::Separator::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    let editor = build_editor(ui);

    root.append(&sidebar);
    root.append(&sidebar_separator);
    root.append(&editor);

    let shell = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    content.append(&ui.edit_revealer);
    content.append(&root);

    shell.append(&header_bar);
    shell.append(&content);

    window.set_content(Some(&shell));
    window
}

fn drift_logo_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../orbital-assets/logos/drift_logo.png"
    )
}

fn install_app_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        .header-action {
            border-radius: 999px;
            border: 1px solid alpha(currentColor, 0.16);
            background: alpha(currentColor, 0.03);
            padding: 4px 12px;
        }

        .header-action:hover {
            background: alpha(currentColor, 0.06);
        }

        .header-action:active,
        .header-action:checked {
            background: alpha(currentColor, 0.09);
            border-color: alpha(currentColor, 0.22);
        }

        .page-drop-indicator {
            min-height: 3px;
            border-radius: 999px;
            background: alpha(currentColor, 0.22);
        }

        row.page-row:drop(active),
        row.page-row:drop(active):hover,
        row.page-row:drop(active):selected,
        .page-row-target:drop(active),
        .page-row-target:drop(active):hover,
        .page-row-target:drop(active):selected {
            background: transparent;
            box-shadow: none;
            outline: none;
            border: none;
        }
        ",
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_settings_window(
    parent: &adw::ApplicationWindow,
    ui: &Rc<DriftUi>,
) -> adw::PreferencesWindow {
    let window = adw::PreferencesWindow::builder()
        .title("Drift Einstellungen")
        .transient_for(parent)
        .search_enabled(false)
        .default_width(760)
        .default_height(560)
        .build();

    let general_page = adw::PreferencesPage::builder()
        .title("Allgemein")
        .icon_name("preferences-system-symbolic")
        .build();
    let personalization_page = adw::PreferencesPage::builder()
        .title("Personalisierung")
        .icon_name("applications-graphics-symbolic")
        .build();
    let hotkeys_page = adw::PreferencesPage::builder()
        .title("Hotkeys")
        .icon_name("input-keyboard-symbolic")
        .build();
    let help_page = adw::PreferencesPage::builder()
        .title("Hilfe")
        .icon_name("help-browser-symbolic")
        .build();

    let general_group = adw::PreferencesGroup::builder()
        .title("Canvas")
        .description("Grundlegende Einstellungen fur den blockbasierten Editor.")
        .build();
    let grid_row = adw::ActionRow::builder()
        .title("Grid-Feinheit")
        .subtitle("Legt fest, wie fein das Raster dargestellt wird und woran Blocks einrasten.")
        .build();
    let labels = settings::grid_density_labels();
    let grid_dropdown = gtk::DropDown::from_strings(&labels);
    grid_dropdown.set_valign(gtk::Align::Center);
    grid_dropdown.set_selected(ui.settings.borrow().grid_density.index());
    grid_row.add_suffix(&grid_dropdown);
    grid_row.set_activatable_widget(Some(&grid_dropdown));
    general_group.add(&grid_row);
    general_page.add(&general_group);

    {
        let ui = Rc::clone(ui);
        grid_dropdown.connect_selected_notify(move |dropdown| {
            let grid_density = settings::GridDensity::from_index(dropdown.selected());
            if grid_density == ui.settings.borrow().grid_density {
                return;
            }

            ui.update_grid_density(grid_density);
        });
    }

    let personalization_group = adw::PreferencesGroup::builder()
        .title("Personalisierung")
        .description("Hier kommt spater die optische Anpassung von Drift hinein.")
        .build();
    personalization_group.add(&info_row(
        "Editor-Design",
        "Weitere Farben, Block-Stile und Ansichten folgen in einem spateren Schritt.",
    ));
    personalization_page.add(&personalization_group);

    let hotkeys_group = adw::PreferencesGroup::builder()
        .title("Tastatur")
        .description("Die wichtigsten Arbeitsablaufe fur Drift.")
        .build();
    hotkeys_group.add(&info_row(
        "Textformatierung",
        "Formatierungs-Shortcuts und frei konfigurierbare Hotkeys kommen spater dazu.",
    ));
    hotkeys_group.add(&info_row(
        "Navigation",
        "Seitenwechsel, Block-Steuerung und Schnellaktionen werden hier gesammelt.",
    ));
    hotkeys_page.add(&hotkeys_group);

    let help_group = adw::PreferencesGroup::builder()
        .title("Hilfe")
        .description("Kurzubersicht zum aktuellen Stand von Drift.")
        .build();
    help_group.add(&info_row(
        "Drift aktuell",
        "Notizen laufen lokal, blockbasiert und ohne Cloud. Weitere Blocktypen folgen schrittweise.",
    ));
    help_group.add(&info_row(
        "Grid-Option",
        "Die Grid-Feinheit in Allgemein verwendet aktuell den derzeitigen Rastergrad als Standard.",
    ));
    help_page.add(&help_group);

    window.add(&general_page);
    window.add(&personalization_page);
    window.add(&hotkeys_page);
    window.add(&help_page);
    window
}

fn info_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build()
}

fn build_formatting_toolbar(ui: &Rc<DriftUi>) -> gtk::Box {
    let toolbar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(16)
        .margin_end(16)
        .hexpand(true)
        .build();
    toolbar.add_css_class("toolbar");

    let style_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    let paragraph_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    let insert_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    let color_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();

    let bold_button = gtk::ToggleButton::new();
    let italic_button = gtk::ToggleButton::new();
    let underline_button = gtk::ToggleButton::new();
    let strike_button = gtk::ToggleButton::new();
    let clear_button = gtk::Button::with_label("Clear");
    let paragraph_label = gtk::Label::builder().label("Style").xalign(0.0).build();
    let paragraph_combo = gtk::ComboBoxText::new();
    let bullet_button = gtk::Button::with_label("List");
    let checklist_button = gtk::Button::with_label("Checklist");
    let color_label = gtk::Label::builder().label("Color").xalign(0.0).build();
    let color_combo = gtk::ComboBoxText::new();

    bold_button.add_css_class("pill");
    italic_button.add_css_class("pill");
    underline_button.add_css_class("pill");
    strike_button.add_css_class("pill");
    clear_button.add_css_class("pill");
    checklist_button.add_css_class("pill");
    bullet_button.add_css_class("pill");
    paragraph_combo.append(Some("normal"), "Normal");
    paragraph_combo.append(Some("heading1"), "Heading 1");
    paragraph_combo.set_active_id(Some("normal"));
    color_combo.append(Some("default"), "Default");
    color_combo.append(Some("red"), "Red");
    color_combo.append(Some("blue"), "Blue");
    color_combo.append(Some("green"), "Green");
    color_combo.append(Some("orange"), "Orange");
    color_combo.set_active_id(Some("default"));

    bold_button.set_child(Some(&styled_toolbar_label("<b>B</b>")));
    italic_button.set_child(Some(&styled_toolbar_label("<i>I</i>")));
    underline_button.set_child(Some(&styled_toolbar_label("<u>U</u>")));
    strike_button.set_child(Some(&styled_toolbar_label(
        "<span strikethrough=\"true\">S</span>",
    )));

    connect_style_toggle(&bold_button, ui, |ui, active| {
        ui.bold_mode.set(active);
        ui.active_buffer()
            .map(|buffer| rich_text::set_bold(&buffer, active))
            .unwrap_or(false)
    });
    connect_style_toggle(&italic_button, ui, |ui, active| {
        ui.italic_mode.set(active);
        ui.active_buffer()
            .map(|buffer| rich_text::set_italic(&buffer, active))
            .unwrap_or(false)
    });
    connect_style_toggle(&underline_button, ui, |ui, active| {
        ui.underline_mode.set(active);
        ui.active_buffer()
            .map(|buffer| rich_text::set_underline(&buffer, active))
            .unwrap_or(false)
    });
    connect_style_toggle(&strike_button, ui, |ui, active| {
        ui.strikethrough_mode.set(active);
        ui.active_buffer()
            .map(|buffer| rich_text::set_strikethrough(&buffer, active))
            .unwrap_or(false)
    });
    let bold_button_for_clear = bold_button.clone();
    let italic_button_for_clear = italic_button.clone();
    let underline_button_for_clear = underline_button.clone();
    let strike_button_for_clear = strike_button.clone();
    let color_combo_for_clear = color_combo.clone();

    connect_toolbar_action(&clear_button, ui, move |ui| {
        ui.bold_mode.set(false);
        ui.italic_mode.set(false);
        ui.underline_mode.set(false);
        ui.strikethrough_mode.set(false);
        ui.color_mode.replace(None);
        ui.loading_ui.set(true);
        bold_button_for_clear.set_active(false);
        italic_button_for_clear.set_active(false);
        underline_button_for_clear.set_active(false);
        strike_button_for_clear.set_active(false);
        color_combo_for_clear.set_active_id(Some("default"));
        ui.loading_ui.set(false);
        ui.active_buffer()
            .map(|buffer| rich_text::clear_formatting(&buffer))
            .unwrap_or(false)
    });
    connect_toolbar_action(&bullet_button, ui, |ui| {
        ui.active_buffer()
            .map(|buffer| insert_bullet_list(&buffer))
            .unwrap_or(false)
    });
    {
        let ui = Rc::clone(ui);
        checklist_button.connect_clicked(move |_| {
            if let Err(error) = ui.apply_checklist() {
                ui.set_status(&format!("Checklist failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        paragraph_combo.connect_changed(move |combo| {
            let style = match combo.active_id().as_deref() {
                Some("heading1") => rich_text::ParagraphStyle::Heading1,
                _ => rich_text::ParagraphStyle::Normal,
            };

            let result = match style {
                rich_text::ParagraphStyle::Heading1 => {
                    ui.apply_paragraph_style(style, "Heading saved")
                }
                rich_text::ParagraphStyle::Normal => {
                    ui.apply_paragraph_style(style, "Paragraph saved")
                }
            };

            if let Err(error) = result {
                ui.set_status(&format!("Paragraph style failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        color_combo.connect_changed(move |combo| {
            let Some(color_id) = combo.active_id() else {
                return;
            };

            let changed = if color_id.as_str() == "default" {
                ui.color_mode.replace(None);
                if let Err(error) = ui.record_history_checkpoint() {
                    ui.set_status(&format!("History checkpoint failed: {error}"));
                    return;
                }
                ui.active_buffer()
                    .map(|buffer| rich_text::set_color(&buffer, None))
                    .unwrap_or(false)
            } else {
                ui.color_mode.replace(Some(color_id.to_string()));
                if let Err(error) = ui.record_history_checkpoint() {
                    ui.set_status(&format!("History checkpoint failed: {error}"));
                    return;
                }
                ui.active_buffer()
                    .map(|buffer| rich_text::set_color(&buffer, Some(color_id.as_str())))
                    .unwrap_or(false)
            };

            if changed {
                ui.mark_dirty();
                if let Err(error) = ui.save_immediately("Formatting saved") {
                    ui.set_status(&format!("Formatting save failed: {error}"));
                }
            } else {
                ui.set_status("Typing color updated");
            }
        });
    }

    style_group.append(&bold_button);
    style_group.append(&italic_button);
    style_group.append(&underline_button);
    style_group.append(&strike_button);
    style_group.append(&clear_button);

    paragraph_group.append(&paragraph_label);
    paragraph_group.append(&paragraph_combo);

    insert_group.append(&bullet_button);
    insert_group.append(&checklist_button);

    color_group.append(&color_label);
    color_group.append(&color_combo);

    toolbar.append(&style_group);
    toolbar.append(&gtk::Separator::builder().orientation(gtk::Orientation::Vertical).build());
    toolbar.append(&paragraph_group);
    toolbar.append(&gtk::Separator::builder().orientation(gtk::Orientation::Vertical).build());
    toolbar.append(&insert_group);
    toolbar.append(&gtk::Separator::builder().orientation(gtk::Orientation::Vertical).build());
    toolbar.append(&color_group);
    toolbar
}

fn build_sidebar(ui: &Rc<DriftUi>) -> gtk::Box {
    let sidebar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .width_request(if ui.sidebar_collapsed.get() {
            SIDEBAR_COLLAPSED_WIDTH
        } else {
            SIDEBAR_EXPANDED_WIDTH
        })
        .build();
    sidebar.set_hexpand(false);
    sidebar.set_halign(gtk::Align::Start);

    let notebook_title = gtk::Label::builder()
        .label("Notebook")
        .xalign(0.0)
        .build();
    notebook_title.add_css_class("title-4");

    let notebook_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .margin_top(4)
        .margin_bottom(4)
        .margin_start(10)
        .margin_end(10)
        .build();
    notebook_box.append(&notebook_title);

    let notebook_frame = gtk::Frame::new(None);
    notebook_frame.set_child(Some(&notebook_box));
    notebook_frame.add_css_class("card");
    notebook_frame.set_halign(gtk::Align::Start);

    let toggle_button = gtk::Button::builder()
        .icon_name(if ui.sidebar_collapsed.get() {
            "go-next-symbolic"
        } else {
            "go-previous-symbolic"
        })
        .tooltip_text(if ui.sidebar_collapsed.get() {
            "Expand sidebar"
        } else {
            "Collapse sidebar"
        })
        .build();
    toggle_button.add_css_class("header-action");

    let header_spacer = gtk::Box::builder().hexpand(true).build();
    let header_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    header_row.set_halign(gtk::Align::Fill);
    header_row.append(&notebook_frame);
    header_row.append(&header_spacer);
    header_row.append(&toggle_button);

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&ui.list_box)
        .build();
    scroller.set_hexpand(false);
    scroller.set_min_content_width(if ui.sidebar_collapsed.get() {
        SIDEBAR_COLLAPSED_WIDTH - 32
    } else {
        SIDEBAR_EXPANDED_WIDTH - 32
    });

    {
        let ui = Rc::clone(ui);
        let sidebar = sidebar.clone();
        let scroller = scroller.clone();
        let button_signal = toggle_button.clone();
        let button_state = toggle_button.clone();
        let collapsed = Rc::new(Cell::new(ui.sidebar_collapsed.get()));
        let collapsed_state = Rc::clone(&collapsed);

        button_signal.connect_clicked(move |_| {
            let next_collapsed = !collapsed_state.get();
            collapsed_state.set(next_collapsed);
            ui.sidebar_collapsed.set(next_collapsed);

            sidebar.set_width_request(if next_collapsed {
                SIDEBAR_COLLAPSED_WIDTH
            } else {
                SIDEBAR_EXPANDED_WIDTH
            });
            scroller.set_min_content_width(if next_collapsed {
                1
            } else {
                SIDEBAR_EXPANDED_WIDTH - 32
            });

            if next_collapsed {
                button_state.set_icon_name("go-next-symbolic");
                button_state.set_tooltip_text(Some("Expand sidebar"));
            } else {
                button_state.set_icon_name("go-previous-symbolic");
                button_state.set_tooltip_text(Some("Collapse sidebar"));
            }

            if let Err(error) = ui.refresh_note_summaries(ui.selected_note_id.borrow().clone()) {
                ui.set_status(&format!("Sidebar update failed: {error}"));
            }
        });
    }

    sidebar.append(&header_row);
    sidebar.append(&scroller);
    sidebar
}

fn build_editor(ui: &Rc<DriftUi>) -> gtk::Box {
    let editor = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .hexpand(true)
        .vexpand(true)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let page_title = gtk::Label::builder()
        .label("Page")
        .xalign(0.0)
        .build();
    page_title.add_css_class("title-4");

    let title_entry = ui.title_entry.clone();
    title_entry.add_css_class("title-2");

    let canvas_grid = ui.canvas_grid.clone();
    let ui_for_draw = Rc::clone(ui);
    canvas_grid.set_draw_func(move |_, cr, width, height| {
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.paint();

        let major_step = ui_for_draw.grid_size().max(1);
        let minor_step = (major_step / 2).max(1);
        let minor_step_usize = minor_step as usize;

        for y in (0..height).step_by(minor_step_usize) {
            for x in (0..width).step_by(minor_step_usize) {
                let is_major = x % major_step == 0 && y % major_step == 0;
                let radius = if is_major { 1.2 } else { 0.7 };
                let alpha = if is_major { 0.12 } else { 0.06 };

                cr.set_source_rgba(0.0, 0.0, 0.0, alpha);
                cr.arc(x as f64, y as f64, radius, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
        }
    });

    let canvas_overlay = gtk::Overlay::new();
    canvas_overlay.set_child(Some(&canvas_grid));
    canvas_overlay.add_overlay(&ui.body_canvas_fixed);
    canvas_overlay.set_size_request(block_layout::CANVAS_WIDTH, block_layout::CANVAS_HEIGHT);

    let insert_popover = gtk::Popover::new();
    insert_popover.set_parent(&canvas_overlay);
    insert_popover.set_has_arrow(true);
    insert_popover.set_autohide(true);
    insert_popover.set_position(gtk::PositionType::Bottom);

    let insert_menu = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    let text_block_button = gtk::Button::with_label("Textblock");
    let image_block_button = gtk::Button::with_label("Bild");
    let code_block_button = gtk::Button::with_label("Code");
    image_block_button.set_sensitive(false);
    code_block_button.set_sensitive(false);

    insert_menu.append(&text_block_button);
    insert_menu.append(&image_block_button);
    insert_menu.append(&code_block_button);
    insert_popover.set_child(Some(&insert_menu));

    let insert_position = Rc::new(RefCell::new((0_i32, 0_i32)));

    ui.body_canvas_fixed.put(&ui.text_block_preview_frame, 0.0, 0.0);
    ui.apply_canvas_layout();

    let body_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&canvas_overlay)
        .build();

    {
        let ui = Rc::clone(ui);
        let insert_popover = insert_popover.clone();
        let insert_position = Rc::clone(&insert_position);
        text_block_button.connect_clicked(move |_| {
            let (x, y) = *insert_position.borrow();
            if let Err(error) = ui.create_text_block_at(x, y) {
                ui.set_status(&format!("Create block failed: {error}"));
            }
            insert_popover.popdown();
        });
    }

    {
        let ui_for_motion = Rc::clone(ui);
        let canvas_surface = ui.body_canvas_fixed.clone();
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(move |controller, x, y| {
            let cursor_name = if ui_for_motion.point_hits_any_block(x, y) {
                None
            } else {
                Some("cell")
            };

            let _ = controller;
            canvas_surface.set_cursor_from_name(cursor_name);
        });

        let canvas_surface = ui.body_canvas_fixed.clone();
        motion.connect_leave(move |_| {
            canvas_surface.set_cursor_from_name(None);
        });

        ui.body_canvas_fixed.add_controller(motion);
    }

    {
        let ui = Rc::clone(ui);
        let ui_for_pan = Rc::clone(&ui);
        let scroller = body_scroller.clone();
        let drag = gtk::GestureDrag::new();
        let pan_origin = Rc::new(RefCell::new(None::<(f64, f64)>));
        let pan_active = Rc::new(Cell::new(false));

        drag.set_button(1);

        {
            let pan_origin = Rc::clone(&pan_origin);
            let pan_active = Rc::clone(&pan_active);
            let scroller = scroller.clone();
            drag.connect_drag_begin(move |_, start_x, start_y| {
                if ui_for_pan.point_hits_any_block(start_x, start_y) {
                    pan_active.set(false);
                    return;
                }

                let hadjustment = scroller.hadjustment();
                let vadjustment = scroller.vadjustment();
                pan_origin.replace(Some((hadjustment.value(), vadjustment.value())));
                pan_active.set(true);
            });
        }

        {
            let pan_origin = Rc::clone(&pan_origin);
            let pan_active = Rc::clone(&pan_active);
            let scroller = scroller.clone();
            drag.connect_drag_update(move |_, offset_x, offset_y| {
                if !pan_active.get() {
                    return;
                }

                let Some((start_h, start_v)) = *pan_origin.borrow() else {
                    return;
                };

                let hadjustment = scroller.hadjustment();
                let vadjustment = scroller.vadjustment();

                let target_h = (start_h - offset_x).clamp(
                    hadjustment.lower(),
                    hadjustment.upper() - hadjustment.page_size(),
                );
                let target_v = (start_v - offset_y).clamp(
                    vadjustment.lower(),
                    vadjustment.upper() - vadjustment.page_size(),
                );

                hadjustment.set_value(target_h);
                vadjustment.set_value(target_v);
            });
        }

        {
            let pan_origin = Rc::clone(&pan_origin);
            let pan_active = Rc::clone(&pan_active);
            drag.connect_drag_end(move |_, _, _| {
                pan_origin.replace(None);
                pan_active.set(false);
            });
        }

        ui.body_canvas_fixed.add_controller(drag);
    }

    {
        let ui = Rc::clone(ui);
        let ui_for_click = Rc::clone(&ui);
        let insert_popover = insert_popover.clone();
        let insert_position = Rc::clone(&insert_position);
        let click = gtk::GestureClick::new();
        click.set_button(3);
        click.connect_pressed(move |_, _, x, y| {
            if ui_for_click.point_hits_any_block(x, y) {
                insert_popover.popdown();
                return;
            }

            let point_x = x.round() as i32;
            let point_y = y.round() as i32;
            insert_position.replace((point_x, point_y));
            insert_popover
                .set_pointing_to(Some(&gtk::gdk::Rectangle::new(point_x, point_y, 1, 1)));
            insert_popover.popup();
        });

        ui.body_canvas_fixed.add_controller(click);
    }

    editor.append(&page_title);
    editor.append(&title_entry);
    editor.append(&body_scroller);
    editor.append(&ui.status_label);
    editor
}

fn connect_actions(
    ui: &Rc<DriftUi>,
    new_button: &gtk::Button,
    undo_button: &gtk::Button,
    redo_button: &gtk::Button,
    edit_button: &gtk::ToggleButton,
    settings_button: &gtk::Button,
    window: &adw::ApplicationWindow,
) {
    {
        let ui = Rc::clone(ui);
        new_button.connect_clicked(move |_| {
            if let Err(error) = ui.create_note(true) {
                ui.set_status(&format!("Create failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        edit_button.connect_toggled(move |button| {
            ui.edit_revealer.set_reveal_child(button.is_active());
        });
    }

    {
        let ui = Rc::clone(ui);
        undo_button.connect_clicked(move |_| {
            if let Err(error) = ui.handle_undo() {
                ui.set_status(&format!("Undo failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        redo_button.connect_clicked(move |_| {
            if let Err(error) = ui.handle_redo() {
                ui.set_status(&format!("Redo failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        let parent_window = window.clone();
        settings_button.connect_clicked(move |_| {
            let settings_window = build_settings_window(&parent_window, &ui);
            settings_window.present();
        });
    }

    {
        let ui = Rc::clone(ui);
        let title_entry = ui.title_entry.clone();
        let title_focus = gtk::EventControllerFocus::new();

        {
            let ui = Rc::clone(&ui);
            title_focus.connect_leave(move |_| {
                ui.title_history_pending.set(true);
            });
        }

        title_entry.add_controller(title_focus);

        title_entry.connect_changed(move |_| {
            if ui.loading_ui.get() {
                return;
            }

            if ui.title_history_pending.replace(false) {
                if let Err(error) = ui.record_history_checkpoint() {
                    ui.set_status(&format!("History checkpoint failed: {error}"));
                    ui.title_history_pending.set(true);
                    return;
                }
            }

            ui.mark_dirty();
            ui.schedule_autosave();
        });
    }

    {
        let ui = Rc::clone(ui);
        let list_box = ui.list_box.clone();

        list_box.connect_row_selected(move |_, row| {
            if ui.loading_ui.get() {
                return;
            }

            let Some(row) = row else {
                ui.clear_editor();
                return;
            };

            if let Err(error) = ui.load_note_into_editor(row.index() as usize) {
                ui.set_status(&format!("Load failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        window.connect_close_request(move |_| {
            if let Err(error) = ui.flush_autosave() {
                ui.set_status(&format!("Final save failed: {error}"));
            }

            glib::Propagation::Proceed
        });
    }

    {
        let ui = Rc::clone(ui);
        let key = gtk::EventControllerKey::new();
        key.connect_key_pressed(move |_, keyval, _, state| match ui.handle_shortcut(keyval, state) {
            Ok(true) => glib::Propagation::Stop,
            Ok(false) => glib::Propagation::Proceed,
            Err(error) => {
                ui.set_status(&format!("Shortcut failed: {error}"));
                glib::Propagation::Stop
            }
        });
        window.add_controller(key);
    }
}

fn connect_toolbar_action<F>(button: &gtk::Button, ui: &Rc<DriftUi>, action: F)
where
    F: Fn(&DriftUi) -> bool + 'static,
{
    let ui = Rc::clone(ui);

    button.connect_clicked(move |_| {
        if let Err(error) = ui.record_history_checkpoint() {
            ui.set_status(&format!("History checkpoint failed: {error}"));
            return;
        }

        if action(&ui) {
            ui.mark_dirty();
            if let Err(error) = ui.save_immediately("Formatting saved") {
                ui.set_status(&format!("Formatting save failed: {error}"));
            }
        } else {
            ui.set_status("Select text first");
        }
    });
}

fn connect_style_toggle<F>(button: &gtk::ToggleButton, ui: &Rc<DriftUi>, action: F)
where
    F: Fn(&DriftUi, bool) -> bool + 'static,
{
    let ui = Rc::clone(ui);

    button.connect_toggled(move |button| {
        if ui.loading_ui.get() {
            return;
        }

        let active = button.is_active();
        if let Err(error) = ui.record_history_checkpoint() {
            ui.set_status(&format!("History checkpoint failed: {error}"));
            return;
        }
        let changed_selection = action(&ui, active);

        if changed_selection {
            ui.mark_dirty();
            if let Err(error) = ui.save_immediately("Formatting saved") {
                ui.set_status(&format!("Formatting save failed: {error}"));
            }
        } else if active {
            ui.set_status("Typing mode enabled");
        } else {
            ui.set_status("Typing mode disabled");
        }
    });
}

fn styled_toolbar_label(markup: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_use_markup(true);
    label.set_markup(markup);
    label
}

fn insert_bullet_list(buffer: &gtk::TextBuffer) -> bool {
    if insert_line_prefix_on_empty_line(buffer, "- ") {
        return true;
    }

    transform_selected_lines(buffer, |line| {
        if line.trim().is_empty() {
            String::new()
        } else if line.trim_start().starts_with("- ") {
            line.to_string()
        } else {
            format!("- {line}")
        }
    })
}

fn insert_checklist(buffer: &gtk::TextBuffer) -> bool {
    if insert_line_prefix_on_empty_line(buffer, "- [ ] ") {
        return true;
    }

    transform_selected_lines(buffer, |line| {
        if line.trim().is_empty() {
            String::new()
        } else if line.trim_start().starts_with("- [ ] ") || line.trim_start().starts_with("- [x] ") {
            line.to_string()
        } else {
            format!("- [ ] {line}")
        }
    })
}

fn continue_list_or_checklist(buffer: &gtk::TextBuffer) -> bool {
    let insert_mark = buffer.get_insert();
    let insert = buffer.iter_at_mark(&insert_mark);
    let mut line_start = insert;
    line_start.set_line_offset(0);
    let mut line_end = line_start;
    line_end.forward_to_line_end();
    let current_line = buffer.text(&line_start, &line_end, true).to_string();

    let continuation = if let Some(rest) = current_line.strip_prefix("- [ ] ") {
        if rest.trim().is_empty() {
            Some(String::new())
        } else {
            Some("\n- [ ] ".to_string())
        }
    } else if let Some(rest) = current_line.strip_prefix("- [x] ") {
        if rest.trim().is_empty() {
            Some(String::new())
        } else {
            Some("\n- [ ] ".to_string())
        }
    } else if let Some(rest) = current_line.strip_prefix("- ") {
        if rest.trim().is_empty() {
            Some(String::new())
        } else {
            Some("\n- ".to_string())
        }
    } else {
        None
    };

    let Some(insert_text) = continuation else {
        return false;
    };

    buffer.begin_user_action();
    let mut insert_at = buffer.iter_at_mark(&insert_mark);
    if insert_text.is_empty() {
        let mut delete_start = line_start;
        let mut delete_end = line_end;
        buffer.delete(&mut delete_start, &mut delete_end);
    } else {
        buffer.insert(&mut insert_at, &insert_text);
    }
    buffer.end_user_action();
    true
}

fn insert_line_prefix_on_empty_line(buffer: &gtk::TextBuffer, prefix: &str) -> bool {
    if buffer.selection_bounds().is_some() {
        return false;
    }

    let insert_mark = buffer.get_insert();
    let mut line_start = buffer.iter_at_mark(&insert_mark);
    line_start.set_line_offset(0);
    let mut line_end = line_start;
    line_end.forward_to_line_end();
    let current_line = buffer.text(&line_start, &line_end, true).to_string();

    if !current_line.trim().is_empty() {
        return false;
    }

    buffer.begin_user_action();
    buffer.insert(&mut line_start, prefix);
    buffer.end_user_action();
    true
}

fn transform_selected_lines<F>(buffer: &gtk::TextBuffer, transform: F) -> bool
where
    F: Fn(&str) -> String,
{
    let (mut start, end) = if let Some((start, end)) = buffer.selection_bounds() {
        (start, end)
    } else {
        let insert_mark = buffer.get_insert();
        let mut start = buffer.iter_at_mark(&insert_mark);
        start.set_line_offset(0);
        let mut end = start;
        end.forward_to_line_end();
        (start, end)
    };

    let selected_text = buffer.text(&start, &end, true).to_string();
    if selected_text.trim().is_empty() {
        return false;
    }

    let transformed = selected_text
        .lines()
        .map(transform)
        .collect::<Vec<_>>()
        .join("\n");

    buffer.begin_user_action();
    let mut delete_end = end;
    buffer.delete(&mut start, &mut delete_end);
    buffer.insert(&mut start, &transformed);
    buffer.end_user_action();
    true
}

fn capture_snapshot(buffer: &gtk::TextBuffer) -> EditorSnapshot {
    EditorSnapshot {
        plain_text: buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string(),
        markup: rich_text::serialize_buffer(buffer),
    }
}

fn build_note_row(ui: &Rc<DriftUi>, note: &NoteSummary, collapsed: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(true);
    row.set_activatable(true);
    row.add_css_class("page-row");

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(if collapsed { 0 } else { 6 })
        .build();

    let title = gtk::Label::builder()
        .label(&note.title)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    title.add_css_class("heading");

    content.append(&title);
    if !collapsed {
        let subtitle = gtk::Label::builder()
            .label(if note.excerpt.is_empty() {
                "Empty page"
            } else {
                note.excerpt.as_str()
            })
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(28)
            .build();
        subtitle.add_css_class("dim-label");
        content.append(&subtitle);
    }

    let drag_handle = gtk::Button::builder()
        .icon_name("list-drag-handle-symbolic")
        .tooltip_text("Reorder page")
        .build();
    drag_handle.add_css_class("flat");
    drag_handle.set_opacity(0.0);

    let row_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(if collapsed { 8 } else { 10 })
        .margin_bottom(if collapsed { 8 } else { 10 })
        .margin_start(10)
        .margin_end(10)
        .build();
    let row_spacer = gtk::Box::builder().hexpand(true).build();
    row_layout.append(&content);
    row_layout.append(&row_spacer);
    row_layout.append(&drag_handle);

    let top_indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    top_indicator.set_height_request(3);
    top_indicator.set_margin_start(10);
    top_indicator.set_margin_end(10);
    top_indicator.set_opacity(0.0);
    top_indicator.add_css_class("page-drop-indicator");

    let bottom_indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bottom_indicator.set_height_request(3);
    bottom_indicator.set_margin_start(10);
    bottom_indicator.set_margin_end(10);
    bottom_indicator.set_opacity(0.0);
    bottom_indicator.add_css_class("page-drop-indicator");

    let row_wrapper = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    row_wrapper.add_css_class("page-row-target");
    row_wrapper.append(&top_indicator);
    row_wrapper.append(&row_layout);
    row_wrapper.append(&bottom_indicator);
    row.set_child(Some(&row_wrapper));

    {
        let drag_handle = drag_handle.clone();
        let hover = gtk::EventControllerMotion::new();
        let drag_handle_enter = drag_handle.clone();
        hover.connect_enter(move |_, _, _| {
            drag_handle_enter.set_opacity(1.0);
        });

        let drag_handle_leave = drag_handle.clone();
        hover.connect_leave(move |_| {
            drag_handle_leave.set_opacity(0.0);
        });
        row_wrapper.add_controller(hover);
    }

    let menu_popover = gtk::Popover::new();
    menu_popover.set_parent(&ui.list_box);
    menu_popover.set_has_arrow(true);
    menu_popover.set_autohide(true);
    menu_popover.set_position(gtk::PositionType::Bottom);

    let menu_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    let rename_button = gtk::Button::with_label("Rename");
    let create_subpage_button = gtk::Button::with_label("Create SubPage");
    let move_as_subpage_button = gtk::Button::with_label("Move as SubPage");
    let duplicate_button = gtk::Button::with_label("Duplicate");
    let remove_button = gtk::Button::with_label("Remove");
    remove_button.add_css_class("destructive-action");
    create_subpage_button.set_sensitive(false);
    move_as_subpage_button.set_sensitive(false);

    menu_box.append(&rename_button);
    menu_box.append(&create_subpage_button);
    menu_box.append(&move_as_subpage_button);
    menu_box.append(&duplicate_button);
    menu_box.append(&remove_button);
    menu_popover.set_child(Some(&menu_box));

    {
        let ui = Rc::clone(ui);
        let menu_popover = menu_popover.clone();
        let note_id = note.id.clone();
        rename_button.connect_clicked(move |_| {
            if let Err(error) = ui.begin_page_rename(&note_id) {
                ui.set_status(&format!("Rename failed: {error}"));
            }
            menu_popover.popdown();
        });
    }

    {
        let ui = Rc::clone(ui);
        let menu_popover = menu_popover.clone();
        create_subpage_button.connect_clicked(move |_| {
            ui.set_status("Create SubPage comes next");
            menu_popover.popdown();
        });
    }

    {
        let ui = Rc::clone(ui);
        let menu_popover = menu_popover.clone();
        move_as_subpage_button.connect_clicked(move |_| {
            ui.set_status("Move as SubPage comes next");
            menu_popover.popdown();
        });
    }

    {
        let ui = Rc::clone(ui);
        let menu_popover = menu_popover.clone();
        let note_id = note.id.clone();
        duplicate_button.connect_clicked(move |_| {
            if let Err(error) = ui.duplicate_page(&note_id) {
                ui.set_status(&format!("Duplicate failed: {error}"));
            }
            menu_popover.popdown();
        });
    }

    {
        let ui = Rc::clone(ui);
        let menu_popover = menu_popover.clone();
        let note_id = note.id.clone();
        remove_button.connect_clicked(move |_| {
            if let Err(error) = ui.remove_page(&note_id) {
                ui.set_status(&format!("Remove failed: {error}"));
            }
            menu_popover.popdown();
        });
    }

    {
        let menu_popover = menu_popover.clone();
        let row_for_menu = row.clone();
        let right_click = gtk::GestureClick::new();
        right_click.set_button(3);
        right_click.connect_pressed(move |_, _, x, y| {
            let allocation = row_for_menu.allocation();
            menu_popover
                .set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                    allocation.x() + x as i32,
                    allocation.y() + y as i32,
                    1,
                    1,
                )));
            menu_popover.popup();
        });
        row_wrapper.add_controller(right_click);
    }

    {
        let source_id = note.id.to_string();
        let drag_source = gtk::DragSource::builder()
            .actions(gtk::gdk::DragAction::MOVE)
            .build();
        drag_source.connect_prepare(move |_, _, _| {
            Some(gtk::gdk::ContentProvider::for_value(&source_id.to_value()))
        });
        drag_handle.add_controller(drag_source);
    }

    {
        let ui = Rc::clone(ui);
        let target_id = note.id.clone();
        let drop_target = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let row_layout_for_drop = row_layout.clone();
        let top_indicator_for_motion = top_indicator.clone();
        let bottom_indicator_for_motion = bottom_indicator.clone();
        drop_target.connect_motion(move |_, _, y| {
            let split = row_layout_for_drop.allocation().height() as f64 / 2.0;
            if y <= split {
                top_indicator_for_motion.set_opacity(1.0);
                bottom_indicator_for_motion.set_opacity(0.0);
            } else {
                top_indicator_for_motion.set_opacity(0.0);
                bottom_indicator_for_motion.set_opacity(1.0);
            }

            gtk::gdk::DragAction::MOVE
        });

        let top_indicator_for_leave = top_indicator.clone();
        let bottom_indicator_for_leave = bottom_indicator.clone();
        drop_target.connect_leave(move |_| {
            top_indicator_for_leave.set_opacity(0.0);
            bottom_indicator_for_leave.set_opacity(0.0);
        });

        let row_layout_for_drop = row_layout.clone();
        let top_indicator_for_drop = top_indicator.clone();
        let bottom_indicator_for_drop = bottom_indicator.clone();
        drop_target.connect_drop(move |_, value, _, y| {
            let Ok(source_id) = value.get::<String>() else {
                return false;
            };

            let place_after = y > (row_layout_for_drop.allocation().height() as f64 / 2.0);
            let source_id = NoteId::new(source_id);
            top_indicator_for_drop.set_opacity(0.0);
            bottom_indicator_for_drop.set_opacity(0.0);

            if let Err(error) = ui.reorder_page(&source_id, &target_id, place_after) {
                ui.set_status(&format!("Reorder failed: {error}"));
                return false;
            }

            true
        });
        row_wrapper.add_controller(drop_target);
    }

    row
}

fn generate_note_id() -> NoteId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    NoteId::new(format!("note-{nanos}"))
}
