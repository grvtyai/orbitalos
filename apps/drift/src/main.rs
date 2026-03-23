use std::cell::{Cell, RefCell};
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
    body_buffer: gtk::TextBuffer,
    canvas_grid: gtk::DrawingArea,
    body_canvas_fixed: gtk::Fixed,
    text_block_frame: gtk::Frame,
    text_block_preview_frame: gtk::Frame,
    text_block_drag_handle: gtk::Box,
    text_block_resize_handle: gtk::Label,
    status_label: gtk::Label,
    edit_revealer: gtk::Revealer,
    notes: RefCell<Vec<NoteSummary>>,
    selected_note_id: RefCell<Option<NoteId>>,
    autosave_source: RefCell<Option<glib::SourceId>>,
    canvas_layout: RefCell<block_layout::NoteCanvasLayout>,
    settings: RefCell<settings::DriftSettings>,
    preview_layout: RefCell<Option<block_layout::TextBlockLayout>>,
    text_block_hovered: Cell<bool>,
    text_block_interacting: Cell<bool>,
    bold_mode: Cell<bool>,
    italic_mode: Cell<bool>,
    underline_mode: Cell<bool>,
    strikethrough_mode: Cell<bool>,
    color_mode: RefCell<Option<String>>,
    dirty: Cell<bool>,
    loading_ui: Cell<bool>,
}

impl DriftUi {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let app_settings = settings::DriftSettings::load(&paths);
        let initial_layout = block_layout::default_note_canvas_layout(app_settings.grid_size());
        let database = OrbitalDatabase::open(&paths)?;

        let new_button = gtk::Button::builder().label("New Page").build();
        let edit_button = gtk::ToggleButton::builder().label("Edit").build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list_box.add_css_class("boxed-list");

        let title_entry = gtk::Entry::builder()
            .placeholder_text("Page title")
            .hexpand(true)
            .build();

        let body_buffer = rich_text::create_buffer();
        let body_view = gtk::TextView::builder()
            .buffer(&body_buffer)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(12)
            .bottom_margin(12)
            .left_margin(12)
            .right_margin(12)
            .monospace(false)
            .vexpand(true)
            .build();

        let body_canvas_fixed = gtk::Fixed::new();
        body_canvas_fixed.set_size_request(block_layout::CANVAS_WIDTH, block_layout::CANVAS_HEIGHT);
        let canvas_grid = gtk::DrawingArea::new();
        canvas_grid.set_content_width(block_layout::CANVAS_WIDTH);
        canvas_grid.set_content_height(block_layout::CANVAS_HEIGHT);

        let text_block_drag_handle = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        text_block_drag_handle.add_css_class("toolbar");

        let text_block_title = gtk::Label::builder()
            .label("Text block")
            .xalign(0.0)
            .hexpand(true)
            .build();
        text_block_title.add_css_class("heading");
        let text_block_hint = gtk::Label::builder()
            .label("Drag here")
            .xalign(1.0)
            .build();
        text_block_hint.add_css_class("dim-label");

        text_block_drag_handle.append(&text_block_title);
        text_block_drag_handle.append(&text_block_hint);
        text_block_drag_handle.set_opacity(0.0);

        let text_block_resize_handle = gtk::Label::builder()
            .label("Resize")
            .halign(gtk::Align::End)
            .margin_top(6)
            .margin_bottom(8)
            .margin_end(10)
            .build();
        text_block_resize_handle.add_css_class("dim-label");
        text_block_resize_handle.set_opacity(0.0);

        let text_block_body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();
        let text_block_scroller = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&body_view)
            .build();
        text_block_body.append(&text_block_drag_handle);
        text_block_body.append(&text_block_scroller);
        text_block_body.append(&text_block_resize_handle);

        let text_block_frame = gtk::Frame::new(None);
        text_block_frame.set_child(Some(&text_block_body));
        let initial_text_block = initial_layout.text_block.clone();
        text_block_frame.set_size_request(
            initial_text_block.width,
            initial_text_block.height,
        );
        text_block_frame.add_css_class("card");

        let text_block_preview_frame = gtk::Frame::new(None);
        let text_block_preview_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text_block_preview_frame.set_child(Some(&text_block_preview_body));
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
            body_buffer,
            canvas_grid,
            body_canvas_fixed,
            text_block_frame,
            text_block_preview_frame,
            text_block_drag_handle,
            text_block_resize_handle,
            status_label,
            edit_revealer,
            notes: RefCell::new(Vec::new()),
            selected_note_id: RefCell::new(None),
            autosave_source: RefCell::new(None),
            canvas_layout: RefCell::new(initial_layout),
            settings: RefCell::new(app_settings),
            preview_layout: RefCell::new(None),
            text_block_hovered: Cell::new(false),
            text_block_interacting: Cell::new(false),
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

        let window = build_window(app, &ui, &new_button, &edit_button, &settings_button);
        connect_actions(&ui, &new_button, &edit_button, &settings_button, &window);
        ui.reload_notes(None)?;

        if ui.notes.borrow().is_empty() {
            ui.create_note()?;
        }

        Ok(window)
    }

    fn reload_notes(&self, preferred_note: Option<NoteId>) -> orbital_core::OrbitalResult<()> {
        let repository = self.repository();
        let notes = repository.list_active()?;
        let selection = preferred_note.or_else(|| self.selected_note_id.borrow().clone());

        self.loading_ui.set(true);
        self.notes.replace(notes.clone());
        self.clear_list_box();

        for note in &notes {
            self.list_box.append(&build_note_row(note));
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

    fn create_note(&self) -> orbital_core::OrbitalResult<()> {
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

    fn refresh_note_summaries(&self, preferred_note: Option<NoteId>) -> orbital_core::OrbitalResult<()> {
        let repository = self.repository();
        let notes = repository.list_active()?;
        let selection = preferred_note.or_else(|| self.selected_note_id.borrow().clone());

        self.loading_ui.set(true);
        self.notes.replace(notes.clone());
        self.clear_list_box();

        for note in &notes {
            self.list_box.append(&build_note_row(note));
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

    fn load_note_into_editor(&self, index: usize) -> orbital_core::OrbitalResult<()> {
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

    fn populate_editor(&self, note: &NoteDocument) {
        self.cancel_autosave();
        self.loading_ui.set(true);
        self.dirty.set(false);
        self.preview_layout.borrow_mut().take();
        self.text_block_interacting.set(false);
        self.text_block_preview_frame.set_visible(false);
        self.text_block_frame.set_opacity(1.0);
        self.title_entry.set_text(&note.summary.title);
        self.canvas_layout
            .replace(block_layout::deserialize_layout(
                note.body_layout.as_deref(),
                self.grid_size(),
            ));
        self.apply_canvas_layout();
        self.sync_text_block_chrome();
        rich_text::set_buffer_content(
            &self.body_buffer,
            &note.body,
            note.body_markup.as_deref(),
        );
        self.loading_ui.set(false);
    }

    fn clear_editor(&self) {
        self.cancel_autosave();
        self.loading_ui.set(true);
        self.dirty.set(false);
        self.selected_note_id.replace(None);
        self.preview_layout.borrow_mut().take();
        self.text_block_interacting.set(false);
        self.text_block_preview_frame.set_visible(false);
        self.text_block_frame.set_opacity(1.0);
        self.title_entry.set_text("");
        self.canvas_layout
            .replace(block_layout::default_note_canvas_layout(self.grid_size()));
        self.apply_canvas_layout();
        self.sync_text_block_chrome();
        rich_text::set_buffer_content(&self.body_buffer, "", None);
        self.loading_ui.set(false);
    }

    fn clear_list_box(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
    }

    fn persist_editor_to_database(&self) -> orbital_core::OrbitalResult<Option<NoteDocument>> {
        let Some(note_id) = self.selected_note_id.borrow().clone() else {
            return Ok(None);
        };

        let title = self.title_entry.text().trim().to_string();
        let body = self
            .body_buffer
            .text(&self.body_buffer.start_iter(), &self.body_buffer.end_iter(), true)
            .to_string();

        let existing = self
            .repository()
            .get(&note_id)?
            .ok_or_else(|| orbital_core::OrbitalError::NotFound {
                entity: "note",
                id: note_id.to_string(),
            })?;

        let note = NoteDocument {
            summary: NoteSummary {
                title: if title.is_empty() {
                    "Untitled page".to_string()
                } else {
                    title
                },
                ..existing.summary
            },
            body,
            body_markup: rich_text::serialize_buffer(&self.body_buffer),
            body_layout: block_layout::serialize_layout(&self.canvas_layout.borrow()),
        };

        let saved = self.repository().save(&note)?;
        self.dirty.set(false);
        Ok(Some(saved))
    }

    fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
    }

    fn apply_canvas_layout(&self) {
        let layout = self
            .canvas_layout
            .borrow()
            .clone()
            .text_block
            .clamp_to_canvas(self.grid_size());

        self.render_text_block_layout(&layout);
    }

    fn render_text_block_layout(&self, layout: &block_layout::TextBlockLayout) {
        self.render_frame_layout(&self.text_block_frame, layout);
    }

    fn render_preview_layout(&self, layout: &block_layout::TextBlockLayout) {
        self.render_frame_layout(&self.text_block_preview_frame, layout);
    }

    fn render_frame_layout(&self, frame: &gtk::Frame, layout: &block_layout::TextBlockLayout) {
        frame.set_size_request(layout.width, layout.height);
        self.body_canvas_fixed.move_(
            frame,
            layout.x as f64,
            layout.y as f64,
        );
    }

    fn update_text_block_layout(&self, new_layout: block_layout::TextBlockLayout) {
        let preview = new_layout
            .preview_constrained(self.grid_size())
            .clamp_to_canvas(self.grid_size());
        self.preview_layout.replace(Some(preview.clone()));
        self.render_preview_layout(&preview);
    }

    fn finalize_text_block_layout(&self) {
        let finalized = self
            .preview_layout
            .borrow()
            .clone()
            .unwrap_or_else(|| self.canvas_layout.borrow().text_block.clone())
            .snapped_to_grid(self.grid_size())
            .clamp_to_canvas(self.grid_size());

        self.canvas_layout.borrow_mut().text_block = finalized.clone();
        self.render_text_block_layout(&finalized);
    }

    fn begin_text_block_interaction(&self) {
        self.text_block_interacting.set(true);
        let current_layout = self
            .canvas_layout
            .borrow()
            .text_block
            .clone()
            .clamp_to_canvas(self.grid_size());
        self.preview_layout.replace(Some(current_layout.clone()));
        self.render_preview_layout(&current_layout);
        self.text_block_preview_frame.set_visible(true);
        self.text_block_frame.set_opacity(0.28);
        self.sync_text_block_chrome();
    }

    fn finish_text_block_interaction(&self) {
        self.preview_layout.borrow_mut().take();
        self.text_block_preview_frame.set_visible(false);
        self.text_block_frame.set_opacity(1.0);
        self.text_block_interacting.set(false);
        self.sync_text_block_chrome();
    }

    fn pending_format(&self) -> rich_text::PendingFormat {
        rich_text::PendingFormat {
            bold: self.bold_mode.get(),
            italic: self.italic_mode.get(),
            underline: self.underline_mode.get(),
            strikethrough: self.strikethrough_mode.get(),
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

    fn sync_text_block_chrome(&self) {
        let visible = self.text_block_hovered.get() || self.text_block_interacting.get();
        let opacity = if visible { 1.0 } else { 0.0 };

        self.text_block_drag_handle.set_opacity(opacity);
        self.text_block_resize_handle.set_opacity(opacity);
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

    fn flush_autosave(&self) -> orbital_core::OrbitalResult<Option<NoteDocument>> {
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

    fn save_immediately(&self, success_message: &str) -> orbital_core::OrbitalResult<()> {
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

fn build_window(
    app: &adw::Application,
    ui: &Rc<DriftUi>,
    new_button: &gtk::Button,
    edit_button: &gtk::ToggleButton,
    settings_button: &gtk::Button,
) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Drift")
        .default_width(1320)
        .default_height(860)
        .build();

    let header_bar = adw::HeaderBar::new();
    let header_title = gtk::Label::builder()
        .label("Drift")
        .xalign(0.0)
        .build();
    header_title.add_css_class("title-3");

    let header_logo = gtk::Image::from_file(drift_logo_path());
    header_logo.set_pixel_size(36);
    header_logo.set_size_request(36, 36);

    let brand_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_end(14)
        .build();
    brand_box.append(&header_logo);
    brand_box.append(&header_title);

    header_bar.pack_start(&brand_box);
    header_bar.pack_start(new_button);
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
    let bullet_button = gtk::Button::with_label("List");
    let color_label = gtk::Label::builder().label("Color").xalign(0.0).build();
    let color_combo = gtk::ComboBoxText::new();

    bold_button.add_css_class("pill");
    italic_button.add_css_class("pill");
    underline_button.add_css_class("pill");
    strike_button.add_css_class("pill");
    clear_button.add_css_class("pill");
    bullet_button.add_css_class("pill");
    color_combo.append(Some("default"), "Default");
    color_combo.append(Some("red"), "Red");
    color_combo.append(Some("blue"), "Blue");
    color_combo.append(Some("green"), "Green");
    color_combo.append(Some("orange"), "Orange");
    color_combo.set_active_id(Some("default"));

    bold_button.set_child(Some(&styled_toolbar_label("<b>Bold</b>")));
    italic_button.set_child(Some(&styled_toolbar_label("<i>Italic</i>")));
    underline_button.set_child(Some(&styled_toolbar_label("<u>Underline</u>")));
    strike_button.set_child(Some(&styled_toolbar_label(
        "<span strikethrough=\"true\">Strike</span>",
    )));

    connect_style_toggle(&bold_button, ui, |ui, active| {
        ui.bold_mode.set(active);
        rich_text::set_bold(&ui.body_buffer, active)
    });
    connect_style_toggle(&italic_button, ui, |ui, active| {
        ui.italic_mode.set(active);
        rich_text::set_italic(&ui.body_buffer, active)
    });
    connect_style_toggle(&underline_button, ui, |ui, active| {
        ui.underline_mode.set(active);
        rich_text::set_underline(&ui.body_buffer, active)
    });
    connect_style_toggle(&strike_button, ui, |ui, active| {
        ui.strikethrough_mode.set(active);
        rich_text::set_strikethrough(&ui.body_buffer, active)
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
        rich_text::clear_formatting(&ui.body_buffer)
    });
    connect_toolbar_action(&bullet_button, ui, |ui| insert_bullet_list(&ui.body_buffer));

    {
        let ui = Rc::clone(ui);
        color_combo.connect_changed(move |combo| {
            let Some(color_id) = combo.active_id() else {
                return;
            };

            let changed = if color_id.as_str() == "default" {
                ui.color_mode.replace(None);
                rich_text::set_color(&ui.body_buffer, None)
            } else {
                ui.color_mode.replace(Some(color_id.to_string()));
                rich_text::set_color(&ui.body_buffer, Some(color_id.as_str()))
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

    {
        let ui = Rc::clone(ui);
        let body_buffer = ui.body_buffer.clone();

        body_buffer.connect_insert_text(move |buffer, location, text| {
            if ui.loading_ui.get() {
                return;
            }

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

    style_group.append(&bold_button);
    style_group.append(&italic_button);
    style_group.append(&underline_button);
    style_group.append(&strike_button);
    style_group.append(&clear_button);

    insert_group.append(&bullet_button);

    color_group.append(&color_label);
    color_group.append(&color_combo);

    toolbar.append(&style_group);
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
        .width_request(320)
        .build();

    let notebook_title = gtk::Label::builder()
        .label("Notebook")
        .xalign(0.0)
        .build();
    notebook_title.add_css_class("title-4");

    let notebook_hint = gtk::Label::builder()
        .label("One local notebook for all pages")
        .xalign(0.0)
        .wrap(true)
        .build();
    notebook_hint.add_css_class("dim-label");

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&ui.list_box)
        .build();

    sidebar.append(&notebook_title);
    sidebar.append(&notebook_hint);
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

    ui.body_canvas_fixed.put(&ui.text_block_frame, 0.0, 0.0);
    ui.body_canvas_fixed.put(&ui.text_block_preview_frame, 0.0, 0.0);
    ui.apply_canvas_layout();

    let body_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&canvas_overlay)
        .build();

    editor.append(&page_title);
    editor.append(&title_entry);
    editor.append(&body_scroller);
    editor.append(&ui.status_label);
    editor
}

fn connect_actions(
    ui: &Rc<DriftUi>,
    new_button: &gtk::Button,
    edit_button: &gtk::ToggleButton,
    settings_button: &gtk::Button,
    window: &adw::ApplicationWindow,
) {
    {
        let ui = Rc::clone(ui);
        new_button.connect_clicked(move |_| {
            if let Err(error) = ui.create_note() {
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
        let parent_window = window.clone();
        settings_button.connect_clicked(move |_| {
            let settings_window = build_settings_window(&parent_window, &ui);
            settings_window.present();
        });
    }

    {
        let ui = Rc::clone(ui);
        let title_entry = ui.title_entry.clone();

        title_entry.connect_changed(move |_| {
            ui.mark_dirty();
            ui.schedule_autosave();
        });
    }

    {
        let ui = Rc::clone(ui);
        let body_buffer = ui.body_buffer.clone();

        body_buffer.connect_changed(move |_| {
            ui.mark_dirty();
            ui.schedule_autosave();
        });
    }

    {
        let ui = Rc::clone(ui);
        let drag_handle = ui.text_block_drag_handle.clone();
        let gesture = gtk::GestureDrag::new();
        let drag_origin = Rc::new(RefCell::new(None::<block_layout::TextBlockLayout>));

        {
            let ui = Rc::clone(&ui);
            let drag_origin = Rc::clone(&drag_origin);
            gesture.connect_drag_begin(move |_, _, _| {
                ui.begin_text_block_interaction();
                drag_origin.replace(Some(ui.canvas_layout.borrow().text_block.clone()));
            });
        }

        {
            let ui = Rc::clone(&ui);
            let drag_origin = Rc::clone(&drag_origin);
            gesture.connect_drag_update(move |_, offset_x, offset_y| {
                let Some(origin) = drag_origin.borrow().clone() else {
                    return;
                };

                ui.update_text_block_layout(block_layout::TextBlockLayout {
                    x: origin.x + offset_x.round() as i32,
                    y: origin.y + offset_y.round() as i32,
                    width: origin.width,
                    height: origin.height,
                });
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

        drag_handle.add_controller(gesture);
    }

    {
        let ui = Rc::clone(ui);
        let resize_handle = ui.text_block_resize_handle.clone();
        let gesture = gtk::GestureDrag::new();
        let resize_origin = Rc::new(RefCell::new(None::<block_layout::TextBlockLayout>));

        {
            let ui = Rc::clone(&ui);
            let resize_origin = Rc::clone(&resize_origin);
            gesture.connect_drag_begin(move |_, _, _| {
                ui.begin_text_block_interaction();
                resize_origin.replace(Some(ui.canvas_layout.borrow().text_block.clone()));
            });
        }

        {
            let ui = Rc::clone(&ui);
            let resize_origin = Rc::clone(&resize_origin);
            gesture.connect_drag_update(move |_, offset_x, offset_y| {
                let Some(origin) = resize_origin.borrow().clone() else {
                    return;
                };

                ui.update_text_block_layout(block_layout::TextBlockLayout {
                    x: origin.x,
                    y: origin.y,
                    width: origin.width + offset_x.round() as i32,
                    height: origin.height + offset_y.round() as i32,
                });
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

        resize_handle.add_controller(gesture);
    }

    {
        let hover = gtk::EventControllerMotion::new();
        let ui_for_enter = Rc::clone(ui);
        hover.connect_enter(move |_, _, _| {
            ui_for_enter.text_block_hovered.set(true);
            ui_for_enter.sync_text_block_chrome();
        });

        let ui_for_leave = Rc::clone(ui);
        hover.connect_leave(move |_| {
            ui_for_leave.text_block_hovered.set(false);
            ui_for_leave.sync_text_block_chrome();
        });

        ui.text_block_frame.add_controller(hover);
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
}

fn connect_toolbar_action<F>(button: &gtk::Button, ui: &Rc<DriftUi>, action: F)
where
    F: Fn(&DriftUi) -> bool + 'static,
{
    let ui = Rc::clone(ui);

    button.connect_clicked(move |_| {
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
    let Some((mut start, end)) = buffer.selection_bounds() else {
        return false;
    };

    let selected_text = buffer.text(&start, &end, true).to_string();

    if selected_text.trim().is_empty() {
        return false;
    }

    let bulleted = selected_text
        .lines()
        .map(|line| format!("- {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    buffer.begin_user_action();
    let mut delete_end = end.clone();
    buffer.delete(&mut start, &mut delete_end);
    buffer.insert(&mut start, &bulleted);
    buffer.end_user_action();

    true
}

fn build_note_row(note: &NoteSummary) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();

    let title = gtk::Label::builder()
        .label(&note.title)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    title.add_css_class("heading");

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

    content.append(&title);
    content.append(&subtitle);
    row.set_child(Some(&content));

    row
}

fn generate_note_id() -> NoteId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    NoteId::new(format!("note-{nanos}"))
}
