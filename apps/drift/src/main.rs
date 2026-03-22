use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::domain::note::{NewNote, NoteDocument, NoteId, NoteSummary};
use orbital_core::{NoteRepository, OrbitalApp, OrbitalDatabase, OrbitalPaths};

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
    notes: RefCell<Vec<NoteSummary>>,
    selected_note_id: RefCell<Option<NoteId>>,
    loading_ui: Cell<bool>,
}

impl DriftUi {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let database = OrbitalDatabase::open(&paths)?;

        let new_button = gtk::Button::builder().label("New Page").build();
        let save_button = gtk::Button::builder().label("Save").build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list_box.add_css_class("boxed-list");

        let title_entry = gtk::Entry::builder()
            .placeholder_text("Page title")
            .hexpand(true)
            .build();

        let body_buffer = gtk::TextBuffer::new(None);
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

        let ui = Rc::new(Self {
            database,
            list_box,
            title_entry,
            body_buffer,
            status_label,
            notes: RefCell::new(Vec::new()),
            selected_note_id: RefCell::new(None),
            loading_ui: Cell::new(false),
        });

        let window = build_window(app, &ui, &new_button, &save_button, &body_view);
        connect_actions(&ui, &new_button, &save_button);
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

    fn save_current_note(&self) -> orbital_core::OrbitalResult<()> {
        let Some(saved) = self.persist_editor_to_database()? else {
            self.set_status("Nothing selected");
            return Ok(());
        };
        self.selected_note_id.replace(Some(saved.summary.id.clone()));
        self.reload_notes(Some(saved.summary.id.clone()))?;
        self.populate_editor(&saved);
        self.set_status("Page saved");

        Ok(())
    }

    fn load_note_into_editor(&self, index: usize) -> orbital_core::OrbitalResult<()> {
        let Some(summary) = self.notes.borrow().get(index).cloned() else {
            return Ok(());
        };

        if let Some(current_id) = self.selected_note_id.borrow().clone() {
            if current_id != summary.id {
                let _ = self.persist_editor_to_database()?;
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
        self.loading_ui.set(true);
        self.title_entry.set_text(&note.summary.title);
        self.body_buffer.set_text(&note.body);
        self.loading_ui.set(false);
    }

    fn clear_editor(&self) {
        self.loading_ui.set(true);
        self.selected_note_id.replace(None);
        self.title_entry.set_text("");
        self.body_buffer.set_text("");
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
        };

        Ok(Some(self.repository().save(&note)?))
    }

    fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
    }

    fn repository(&self) -> NoteRepository<'_> {
        NoteRepository::new(self.database.connection())
    }
}

fn build_window(
    app: &adw::Application,
    ui: &Rc<DriftUi>,
    new_button: &gtk::Button,
    save_button: &gtk::Button,
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
    header_bar.pack_end(save_button);
    header_bar.set_title_widget(Some(&header_title));
    window.set_titlebar(Some(&header_bar));

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

    window.set_content(Some(&root));
    window
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

fn connect_actions(ui: &Rc<DriftUi>, new_button: &gtk::Button, save_button: &gtk::Button) {
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
        save_button.connect_clicked(move |_| {
            if let Err(error) = ui.save_current_note() {
                ui.set_status(&format!("Save failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        ui.list_box.connect_row_selected(move |_, row| {
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
