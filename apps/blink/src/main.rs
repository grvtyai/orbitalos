use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::{
    NewSnapshot, OrbitalApp, OrbitalDatabase, OrbitalPaths, SnapshotId, SnapshotKind,
    SnapshotRepository, SnapshotSummary,
};

mod capture;

const PREVIEW_MAX_WIDTH: i32 = 520;
const PREVIEW_MAX_HEIGHT: i32 = 280;
const PREVIEW_PADDING: i32 = 16;

fn main() {
    adw::init().expect("Failed to initialize Libadwaita");

    let app = adw::Application::builder()
        .application_id(OrbitalApp::Blink.application_id())
        .build();

    app.connect_activate(|app| match BlinkUi::build(app) {
        Ok(window) => window.present(),
        Err(error) => {
            eprintln!("Failed to start Blink: {error}");

            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Blink")
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
                .label("Blink could not start")
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

struct BlinkUi {
    database: OrbitalDatabase,
    paths: OrbitalPaths,
    window: RefCell<Option<adw::ApplicationWindow>>,
    import_dialog: RefCell<Option<gtk::FileChooserNative>>,
    list_box: gtk::ListBox,
    preview_frame: gtk::Frame,
    preview_image: gtk::Picture,
    preview_placeholder: gtk::Label,
    detail_created_at: gtk::Label,
    detail_notes_buffer: gtk::TextBuffer,
    detail_file_path: gtk::Label,
    detail_mime_type: gtk::Label,
    detail_tags_entry: gtk::Entry,
    detail_id: gtk::Label,
    save_button: gtk::Button,
    copy_button: gtk::Button,
    status_label: gtk::Label,
    snapshots: RefCell<Vec<SnapshotSummary>>,
    selected_snapshot_id: RefCell<Option<SnapshotId>>,
}

impl BlinkUi {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let database = OrbitalDatabase::open(&paths)?;
        let app_descriptor = OrbitalApp::Blink.descriptor();

        let header_title = adw::WindowTitle::builder()
            .title(app_descriptor.display_name)
            .subtitle("Phase 1 snapshot library")
            .build();

        let new_button = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("New Snapshot")
            .build();
        new_button.add_css_class("suggested-action");

        let import_button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Import Image")
            .build();

        let capture_button = gtk::Button::builder()
            .icon_name("camera-photo-symbolic")
            .tooltip_text("Capture Screenshot")
            .build();

        let header_bar = adw::HeaderBar::builder()
            .title_widget(&header_title)
            .build();
        header_bar.pack_start(&new_button);
        header_bar.pack_start(&import_button);
        header_bar.pack_start(&capture_button);

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list_box.add_css_class("boxed-list");

        let sidebar_title = gtk::Label::builder()
            .label("Snapshots")
            .xalign(0.0)
            .build();
        sidebar_title.add_css_class("title-4");

        let sidebar_body = gtk::Label::builder()
            .label(
                "Blink stores snapshots through orbital-core and now has a simple \
portal-based screenshot flow for the first real captures.",
            )
            .wrap(true)
            .xalign(0.0)
            .build();
        sidebar_body.add_css_class("dim-label");

        let list_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_width(280)
            .child(&list_box)
            .build();
        list_scroller.set_vexpand(true);
        list_scroller.set_hexpand(true);

        let sidebar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .width_request(320)
            .build();
        sidebar.set_vexpand(true);
        sidebar.append(&sidebar_title);
        sidebar.append(&sidebar_body);
        sidebar.append(&list_scroller);

        let detail_heading = gtk::Label::builder()
            .label("Snapshot Details")
            .xalign(0.0)
            .build();
        detail_heading.add_css_class("title-1");

        let detail_created_at = gtk::Label::builder()
            .label("Created at: -")
            .wrap(true)
            .xalign(0.0)
            .build();
        detail_created_at.add_css_class("dim-label");

        let notes_label = gtk::Label::builder()
            .label("Notes")
            .xalign(0.0)
            .build();
        notes_label.add_css_class("heading");

        let detail_notes_buffer = gtk::TextBuffer::new(None);
        let detail_notes_view = gtk::TextView::builder()
            .buffer(&detail_notes_buffer)
            .wrap_mode(gtk::WrapMode::WordChar)
            .vexpand(false)
            .build();
        detail_notes_view.set_size_request(-1, 120);

        let notes_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(120)
            .child(&detail_notes_view)
            .build();

        let tags_label = gtk::Label::builder()
            .label("Tags")
            .xalign(0.0)
            .build();
        tags_label.add_css_class("heading");

        let detail_tags_entry = gtk::Entry::builder()
            .placeholder_text("tag1, tag2, tag3")
            .hexpand(true)
            .build();

        let save_button = gtk::Button::with_label("Save Changes");
        save_button.add_css_class("suggested-action");

        let copy_button = gtk::Button::with_label("Copy to Clipboard");

        let preview_image = gtk::Picture::new();
        preview_image.set_can_shrink(true);
        preview_image.set_keep_aspect_ratio(true);
        preview_image.set_hexpand(true);
        preview_image.set_vexpand(true);
        preview_image.set_halign(gtk::Align::Center);
        preview_image.set_valign(gtk::Align::Center);

        let preview_placeholder = gtk::Label::builder()
            .label("Import an image to preview it here.")
            .wrap(true)
            .xalign(0.5)
            .justify(gtk::Justification::Center)
            .build();
        preview_placeholder.add_css_class("dim-label");

        let preview_frame = gtk::Frame::new(None);
        preview_frame.set_size_request(PREVIEW_MAX_WIDTH, PREVIEW_MAX_HEIGHT);
        preview_frame.add_css_class("card");

        let preview_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .margin_top(PREVIEW_PADDING)
            .margin_bottom(PREVIEW_PADDING)
            .margin_start(PREVIEW_PADDING)
            .margin_end(PREVIEW_PADDING)
            .build();
        preview_box.set_halign(gtk::Align::Center);
        preview_box.set_valign(gtk::Align::Center);
        preview_box.append(&preview_image);
        preview_box.append(&preview_placeholder);
        preview_frame.set_child(Some(&preview_box));

        let (file_path_row, detail_file_path) = build_info_row("File Path");
        let (mime_type_row, detail_mime_type) = build_info_row("MIME Type");
        let (id_row, detail_id) = build_info_row("Snapshot ID");

        let status_label = gtk::Label::builder()
            .label("Ready")
            .xalign(0.0)
            .build();
        status_label.add_css_class("dim-label");

        let action_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();
        action_box.append(&save_button);
        action_box.append(&copy_button);

        let left_meta = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .build();
        left_meta.append(&notes_label);
        left_meta.append(&notes_scroller);
        left_meta.append(&tags_label);
        left_meta.append(&detail_tags_entry);

        let right_meta = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .build();
        right_meta.append(&file_path_row);
        right_meta.append(&mime_type_row);
        right_meta.append(&id_row);

        let metadata_split = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(24)
            .build();
        metadata_split.append(&left_meta);
        metadata_split.append(&right_meta);

        let detail_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        detail_panel.append(&detail_heading);
        detail_panel.append(&detail_created_at);
        detail_panel.append(&preview_frame);
        detail_panel.append(&action_box);
        detail_panel.append(&metadata_split);
        detail_panel.append(&status_label);

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&header_bar);

        let layout = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .hexpand(true)
            .vexpand(true)
            .build();

        let sidebar_frame = gtk::Frame::new(None);
        sidebar_frame.set_child(Some(&sidebar));
        sidebar_frame.add_css_class("card");
        sidebar_frame.set_width_request(320);
        sidebar_frame.set_hexpand(false);

        let detail_frame = gtk::Frame::new(None);
        detail_frame.set_child(Some(&detail_panel));
        detail_frame.add_css_class("card");
        detail_frame.set_hexpand(true);

        layout.append(&sidebar_frame);
        layout.append(&detail_frame);
        content.append(&layout);

        let ui = Rc::new(Self {
            database,
            paths,
            window: RefCell::new(None),
            import_dialog: RefCell::new(None),
            list_box,
            preview_frame,
            preview_image,
            preview_placeholder,
            detail_created_at,
            detail_notes_buffer,
            detail_file_path,
            detail_mime_type,
            detail_tags_entry,
            detail_id,
            save_button: save_button.clone(),
            copy_button: copy_button.clone(),
            status_label,
            snapshots: RefCell::new(Vec::new()),
            selected_snapshot_id: RefCell::new(None),
        });

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(app_descriptor.display_name)
            .default_width(1080)
            .default_height(720)
            .build();
        window.set_content(Some(&content));
        ui.window.replace(Some(window.clone()));

        connect_actions(
            &ui,
            &new_button,
            &import_button,
            &capture_button,
            &save_button,
            &copy_button,
        );
        ui.reload_snapshots(None)?;

        Ok(window)
    }

    fn repository(&self) -> SnapshotRepository<'_> {
        SnapshotRepository::new(self.database.connection())
    }

    fn reload_snapshots(
        self: &Rc<Self>,
        preferred_snapshot: Option<SnapshotId>,
    ) -> orbital_core::OrbitalResult<()> {
        let snapshots = self.repository().list_active()?;
        let selection = preferred_snapshot.or_else(|| self.selected_snapshot_id.borrow().clone());

        self.snapshots.replace(snapshots.clone());
        self.clear_list_box();

        for snapshot in &snapshots {
            self.list_box.append(&build_snapshot_row(self, snapshot));
        }

        if let Some(target) = selection {
            if let Some(index) = snapshots.iter().position(|snapshot| snapshot.id == target) {
                if let Some(row) = self.list_box.row_at_index(index as i32) {
                    self.list_box.select_row(Some(&row));
                    self.load_snapshot_into_detail(index);
                    return Ok(());
                }
            }
        }

        if let Some(first_row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&first_row));
            self.load_snapshot_into_detail(0);
        } else {
            self.show_empty_state();
        }

        Ok(())
    }

    fn create_snapshot(self: &Rc<Self>) -> orbital_core::OrbitalResult<()> {
        let mut new_snapshot =
            NewSnapshot::new(generate_snapshot_id(), "New snapshot", SnapshotKind::Image);
        new_snapshot.source = Some("Blink".to_string());

        let snapshot = self.repository().create(new_snapshot)?;
        self.selected_snapshot_id
            .replace(Some(snapshot.id.clone()));
        self.reload_snapshots(Some(snapshot.id.clone()))?;
        self.set_status("Snapshot created");
        Ok(())
    }

    fn import_image(self: &Rc<Self>, source_path: &Path) -> orbital_core::OrbitalResult<()> {
        let import_dir = self.paths.app_data_dir(OrbitalApp::Blink).join("imports");
        fs::create_dir_all(&import_dir)?;

        let snapshot_id = generate_snapshot_id();
        let extension = source_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        let target_file_name = match extension.as_deref() {
            Some(ext) if !ext.is_empty() => format!("{}.{}", snapshot_id.as_str(), ext),
            _ => snapshot_id.as_str().to_string(),
        };
        let target_path = import_dir.join(target_file_name);

        fs::copy(source_path, &target_path)?;

        let title = source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Imported image");

        self.create_image_snapshot(
            snapshot_id,
            title.to_string(),
            target_path,
            Some(source_path.display().to_string()),
        )?;
        self.set_status("Image imported");
        Ok(())
    }

    fn capture_screenshot(self: &Rc<Self>) {
        self.set_status("Waiting for screenshot selection...");

        let ui = Rc::clone(self);
        gtk::glib::MainContext::default().spawn_local(async move {
            match capture::capture_interactive().await {
                Ok(source_path) => {
                    ui.set_status("Processing screenshot...");
                    if let Err(error) = ui.store_captured_image(&source_path) {
                        ui.set_status(&format!("Capture failed: {error}"));
                    }
                }
                Err(error) => ui.set_status(&error),
            }
        });
    }

    fn store_captured_image(self: &Rc<Self>, source_path: &Path) -> orbital_core::OrbitalResult<()> {
        let capture_dir = self.paths.app_data_dir(OrbitalApp::Blink).join("captures");
        fs::create_dir_all(&capture_dir)?;
        wait_for_file_ready(source_path)?;

        let snapshot_id = generate_snapshot_id();
        let extension = source_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_else(|| "png".to_string());
        let target_path = capture_dir.join(format!("{}.{}", snapshot_id.as_str(), extension));

        fs::copy(source_path, &target_path)?;

        self.create_image_snapshot(
            snapshot_id,
            format!("Screenshot {}", current_timestamp_label()),
            target_path,
            Some("Captured with XDG Desktop Portal".to_string()),
        )?;
        self.set_status("Screenshot captured");
        Ok(())
    }

    fn create_image_snapshot(
        self: &Rc<Self>,
        snapshot_id: SnapshotId,
        title: String,
        stored_path: PathBuf,
        source: Option<String>,
    ) -> orbital_core::OrbitalResult<SnapshotSummary> {
        let mut new_snapshot = NewSnapshot::new(snapshot_id.clone(), title, SnapshotKind::Image);
        new_snapshot.source = source;
        new_snapshot.file_path = Some(stored_path.display().to_string());
        new_snapshot.mime_type = infer_mime_type(&stored_path);

        let snapshot = self.repository().create(new_snapshot)?;
        self.selected_snapshot_id
            .replace(Some(snapshot.id.clone()));
        self.reload_snapshots(Some(snapshot.id.clone()))?;
        Ok(snapshot)
    }

    fn load_snapshot_into_detail(self: &Rc<Self>, index: usize) {
        let Some(snapshot) = self.snapshots.borrow().get(index).cloned() else {
            self.show_empty_state();
            return;
        };

        self.selected_snapshot_id
            .replace(Some(snapshot.id.clone()));
        self.detail_created_at
            .set_label(&format!("Created at: {}", format_timestamp(snapshot.created_at)));
        self.detail_notes_buffer.set_text(&snapshot.notes);
        self.detail_file_path
            .set_label(snapshot.file_path.as_deref().unwrap_or("No file imported yet"));
        self.detail_mime_type
            .set_label(snapshot.mime_type.as_deref().unwrap_or("Unknown"));
        self.detail_tags_entry.set_text(&snapshot.tags.join(", "));
        self.detail_id.set_label(snapshot.id.as_str());
        self.save_button.set_sensitive(true);
        self.copy_button.set_sensitive(snapshot.file_path.is_some());
        self.update_preview(snapshot.file_path.as_deref());
        self.set_status("Snapshot loaded");
    }

    fn save_selected_snapshot(self: &Rc<Self>) -> orbital_core::OrbitalResult<()> {
        let Some(snapshot_id) = self.selected_snapshot_id.borrow().clone() else {
            self.set_status("Select a snapshot first");
            return Ok(());
        };

        let mut snapshot = self
            .repository()
            .get(&snapshot_id)?
            .ok_or(orbital_core::OrbitalError::NotFound {
                entity: "snapshot",
                id: snapshot_id.to_string(),
            })?;

        let notes = self
            .detail_notes_buffer
            .text(
                &self.detail_notes_buffer.start_iter(),
                &self.detail_notes_buffer.end_iter(),
                true,
            )
            .to_string();

        snapshot.notes = notes.trim().to_string();
        snapshot.tags = parse_tags(&self.detail_tags_entry.text());

        let saved = self.repository().save(&snapshot)?;
        self.selected_snapshot_id.replace(Some(saved.id.clone()));
        self.reload_snapshots(Some(saved.id.clone()))?;
        self.set_status("Snapshot saved");
        Ok(())
    }

    fn copy_selected_snapshot_to_clipboard(&self) {
        let Some(snapshot_id) = self.selected_snapshot_id.borrow().clone() else {
            self.set_status("Select a snapshot first");
            return;
        };

        let Ok(Some(snapshot)) = self.repository().get(&snapshot_id) else {
            self.set_status("Copy failed: snapshot could not be loaded");
            return;
        };

        let Some(file_path) = snapshot.file_path.as_deref() else {
            self.set_status("Copy failed: selected snapshot has no image file");
            return;
        };

        let file = gtk::gio::File::for_path(file_path);
        let Ok(texture) = gtk::gdk::Texture::from_file(&file) else {
            self.set_status("Copy failed: image could not be loaded");
            return;
        };

        let Some(display) = gtk::gdk::Display::default() else {
            self.set_status("Copy failed: display is not available");
            return;
        };

        display.clipboard().set_texture(&texture);
        self.set_status("Image copied to clipboard");
    }

    fn remove_snapshot(self: &Rc<Self>, snapshot_id: &SnapshotId) -> orbital_core::OrbitalResult<()> {
        let snapshots = self.snapshots.borrow().clone();
        let fallback_selection = snapshots
            .iter()
            .position(|snapshot| snapshot.id == *snapshot_id)
            .and_then(|index| {
                snapshots
                    .get(index + 1)
                    .or_else(|| index.checked_sub(1).and_then(|previous| snapshots.get(previous)))
            })
            .map(|snapshot| snapshot.id.clone());

        self.repository().archive(snapshot_id)?;
        self.reload_snapshots(fallback_selection)?;
        self.set_status("Snapshot deleted");
        Ok(())
    }

    fn show_empty_state(&self) {
        self.selected_snapshot_id.replace(None);
        self.detail_created_at.set_label("Created at: -");
        self.detail_notes_buffer.set_text("");
        self.detail_file_path.set_label("No file imported yet");
        self.detail_mime_type.set_label("Unknown");
        self.detail_tags_entry.set_text("");
        self.detail_id.set_label("Not created yet");
        self.save_button.set_sensitive(false);
        self.copy_button.set_sensitive(false);
        self.update_preview(None);
        self.set_status("Snapshot library is empty");
    }

    fn clear_list_box(&self) {
        let mut current = self.list_box.first_child();

        while let Some(child) = current {
            let next = child.next_sibling();
            self.list_box.remove(&child);
            current = next;
        }
    }

    fn set_status(&self, message: &str) {
        self.status_label.set_label(message);
    }

    fn update_preview(&self, file_path: Option<&str>) {
        if let Some(path) = file_path {
            let file = gtk::gio::File::for_path(path);
            let Ok(texture) = gtk::gdk::Texture::from_file(&file) else {
                self.preview_image
                    .set_paintable(Option::<&gtk::gdk::Texture>::None);
                self.preview_image.set_visible(false);
                self.preview_placeholder
                    .set_label("Preview could not be loaded for this image.");
                self.preview_placeholder.set_visible(true);
                self.preview_frame
                    .set_size_request(PREVIEW_MAX_WIDTH, PREVIEW_MAX_HEIGHT);
                return;
            };

            let (content_width, content_height) =
                preview_size_for(texture.width(), texture.height());
            self.preview_image.set_paintable(Some(&texture));
            self.preview_image.set_size_request(content_width, content_height);
            self.preview_frame.set_size_request(
                content_width + PREVIEW_PADDING * 2,
                content_height + PREVIEW_PADDING * 2,
            );
            self.preview_image.set_visible(true);
            self.preview_placeholder.set_visible(false);
        } else {
            self.preview_image
                .set_paintable(Option::<&gtk::gdk::Texture>::None);
            self.preview_image
                .set_size_request(PREVIEW_MAX_WIDTH - PREVIEW_PADDING * 2, 1);
            self.preview_frame
                .set_size_request(PREVIEW_MAX_WIDTH, PREVIEW_MAX_HEIGHT);
            self.preview_image.set_visible(false);
            self.preview_placeholder
                .set_label("Import an image to preview it here.");
            self.preview_placeholder.set_visible(true);
        }
    }
}

fn connect_actions(
    ui: &Rc<BlinkUi>,
    new_button: &gtk::Button,
    import_button: &gtk::Button,
    capture_button: &gtk::Button,
    save_button: &gtk::Button,
    copy_button: &gtk::Button,
) {
    {
        let ui = Rc::clone(ui);
        new_button.connect_clicked(move |_| {
            if let Err(error) = ui.create_snapshot() {
                ui.set_status(&format!("Create failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        save_button.connect_clicked(move |_| {
            if let Err(error) = ui.save_selected_snapshot() {
                ui.set_status(&format!("Save failed: {error}"));
            }
        });
    }

    {
        let ui = Rc::clone(ui);
        copy_button.connect_clicked(move |_| {
            ui.copy_selected_snapshot_to_clipboard();
        });
    }

    {
        let ui = Rc::clone(ui);
        import_button.connect_clicked(move |_| {
            let dialog = gtk::FileChooserNative::builder()
                .title("Import Image")
                .action(gtk::FileChooserAction::Open)
                .accept_label("Import")
                .cancel_label("Cancel")
                .build();

            if let Some(window) = ui.window.borrow().clone() {
                dialog.set_transient_for(Some(&window));
            }

            let ui_for_response = Rc::clone(&ui);
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            if let Err(error) = ui_for_response.import_image(&path) {
                                ui_for_response.set_status(&format!("Import failed: {error}"));
                            }
                        } else {
                            ui_for_response
                                .set_status("Import failed: selected file path is not available");
                        }
                    }
                }

                dialog.hide();
                ui_for_response.import_dialog.borrow_mut().take();
            });

            ui.import_dialog.replace(Some(dialog.clone()));
            dialog.show();
        });
    }

    {
        let ui = Rc::clone(ui);
        capture_button.connect_clicked(move |_| {
            ui.capture_screenshot();
        });
    }

    {
        let ui = Rc::clone(ui);
        let list_box = ui.list_box.clone();
        list_box.connect_row_selected(move |_, row| {
            let Some(row) = row else {
                ui.show_empty_state();
                return;
            };

            ui.load_snapshot_into_detail(row.index() as usize);
        });
    }
}

fn build_snapshot_row(ui: &Rc<BlinkUi>, snapshot: &SnapshotSummary) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(true);
    row.set_activatable(true);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();

    let title = gtk::Label::builder()
        .label(&snapshot.title)
        .xalign(0.0)
        .build();
    title.add_css_class("heading");

    let subtitle = gtk::Label::builder()
        .label(match snapshot.file_path.as_deref() {
            Some(path) => format!("{} - {}", snapshot.kind.label(), file_label(path)),
            None => match snapshot.source.as_deref() {
                Some(source) => format!("{} - {source}", snapshot.kind.label()),
                None => snapshot.kind.label().to_string(),
            },
        })
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(28)
        .build();
    subtitle.add_css_class("dim-label");

    content.append(&title);
    content.append(&subtitle);

    let delete_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Delete snapshot")
        .valign(gtk::Align::Center)
        .build();
    delete_button.add_css_class("flat");
    delete_button.add_css_class("destructive-action");
    delete_button.set_opacity(0.0);
    delete_button.set_focus_on_click(false);

    {
        let ui = Rc::clone(ui);
        let snapshot_id = snapshot.id.clone();
        delete_button.connect_clicked(move |_| {
            let dialog = gtk::MessageDialog::builder()
                .transient_for(
                    ui.window
                        .borrow()
                        .as_ref()
                        .expect("Blink window should exist while rows are active"),
                )
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::None)
                .text("Delete?")
                .secondary_text("Do you want to remove this snapshot from the list?")
                .build();

            dialog.add_button("Nein", gtk::ResponseType::No);
            dialog.add_button("Ja", gtk::ResponseType::Yes);
            dialog.set_default_response(gtk::ResponseType::No);

            let ui_for_response = Rc::clone(&ui);
            let snapshot_id_for_response = snapshot_id.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Yes {
                    if let Err(error) = ui_for_response.remove_snapshot(&snapshot_id_for_response) {
                        ui_for_response.set_status(&format!("Delete failed: {error}"));
                    }
                }

                dialog.close();
            });

            dialog.present();
        });
    }

    let row_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();
    content.set_hexpand(true);
    row_layout.append(&content);
    row_layout.append(&delete_button);

    {
        let delete_button_enter = delete_button.clone();
        let hover = gtk::EventControllerMotion::new();
        hover.connect_enter(move |_, _, _| {
            delete_button_enter.set_opacity(1.0);
        });
        let delete_button_leave = delete_button.clone();
        hover.connect_leave(move |_| {
            delete_button_leave.set_opacity(0.0);
        });
        row_layout.add_controller(hover);
    }

    row.set_child(Some(&row_layout));
    row
}

fn build_info_row(label: &str) -> (gtk::Box, gtk::Label) {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();

    let row_label = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .build();
    row_label.add_css_class("heading");

    let row_value = gtk::Label::builder()
        .label("")
        .wrap(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    row_value.add_css_class("dim-label");

    row.append(&row_label);
    row.append(&row_value);
    (row, row_value)
}

fn generate_snapshot_id() -> SnapshotId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    SnapshotId::new(format!("snapshot-{nanos}"))
}

fn infer_mime_type(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();

    let mime_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        _ => return None,
    };

    Some(mime_type.to_string())
}

fn file_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(path)
        .to_string()
}

fn wait_for_file_ready(path: &Path) -> orbital_core::OrbitalResult<()> {
    let mut last_error = None;

    for _ in 0..20 {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                if fs::File::open(path).is_ok() {
                    return Ok(());
                }
            }
            Ok(_) => {
                last_error = Some(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!("Screenshot file is not ready yet: {}", path.display()),
                ));
            }
            Err(error) => {
                last_error = Some(error);
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    Err(last_error
        .unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Screenshot file did not become ready: {}", path.display()),
            )
        })
        .into())
}

fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn format_timestamp(unix_timestamp: i64) -> String {
    gtk::glib::DateTime::from_unix_local(unix_timestamp)
        .and_then(|value| value.format("%d.%m.%Y %H:%M"))
        .map(|value| value.to_string())
        .unwrap_or_else(|_| unix_timestamp.to_string())
}

fn preview_size_for(image_width: i32, image_height: i32) -> (i32, i32) {
    let max_width = (PREVIEW_MAX_WIDTH - PREVIEW_PADDING * 2).max(1) as f64;
    let max_height = (PREVIEW_MAX_HEIGHT - PREVIEW_PADDING * 2).max(1) as f64;
    let width = image_width.max(1) as f64;
    let height = image_height.max(1) as f64;
    let scale = (max_width / width).min(max_height / height).min(1.0);

    let target_width = (width * scale).round() as i32;
    let target_height = (height * scale).round() as i32;

    (target_width.max(1), target_height.max(1))
}

fn current_timestamp_label() -> String {
    gtk::glib::DateTime::now_local()
        .and_then(|value| value.format("%d.%m.%Y %H:%M"))
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "now".to_string())
}
