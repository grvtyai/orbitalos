use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::domain::note::{NewNote, NoteDocument, NoteId, NoteSummary};
use orbital_core::{NoteRepository, OrbitalApp, OrbitalDatabase, OrbitalPaths};

mod rich_text;

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
    list_box: gtk::ListBox,
    title_entry: gtk::Entry,
    body_buffer: gtk::TextBuffer,
    status_label: gtk::Label,
    edit_revealer: gtk::Revealer,
    notes: RefCell<Vec<NoteSummary>>,
    selected_note_id: RefCell<Option<NoteId>>,
    autosave_source: RefCell<Option<glib::SourceId>>,
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
            list_box,
            title_entry,
            body_buffer,
            status_label,
            edit_revealer,
            notes: RefCell::new(Vec::new()),
            selected_note_id: RefCell::new(None),
            autosave_source: RefCell::new(None),
            bold_mode: Cell::new(false),
            italic_mode: Cell::new(false),
            underline_mode: Cell::new(false),
            strikethrough_mode: Cell::new(false),
            color_mode: RefCell::new(None),
            dirty: Cell::new(false),
            loading_ui: Cell::new(false),
        });

        let window = build_window(app, &ui, &new_button, &edit_button, &body_view);
        connect_actions(&ui, &new_button, &edit_button, &window);
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
        let note = repository.create(NewNote::new(
            generate_note_id(),
            "Untitled page",
            "",
        ))?;

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
        self.title_entry.set_text(&note.summary.title);
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
        self.title_entry.set_text("");
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
        };

        let saved = self.repository().save(&note)?;
        self.dirty.set(false);
        Ok(Some(saved))
    }

    fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
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
    body_view: &gtk::TextView,
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
        .build();
    header_title.add_css_class("title-3");

    header_bar.pack_start(new_button);
    header_bar.pack_start(edit_button);
    header_bar.set_title_widget(Some(&header_title));

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
    let editor = build_editor(ui, body_view);

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

fn build_editor(ui: &Rc<DriftUi>, body_view: &gtk::TextView) -> gtk::Box {
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

    let body_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(body_view)
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
