use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::{
    NewSnapshot, OrbitalApp, OrbitalDatabase, OrbitalPaths, SnapshotId, SnapshotKind,
    SnapshotRepository, SnapshotSummary,
};

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
    preview_image: gtk::Image,
    preview_placeholder: gtk::Label,
    detail_title: gtk::Label,
    detail_body: gtk::Label,
    detail_kind: gtk::Label,
    detail_source: gtk::Label,
    detail_file_path: gtk::Label,
    detail_mime_type: gtk::Label,
    detail_tags: gtk::Label,
    detail_id: gtk::Label,
    status_label: gtk::Label,
    snapshots: RefCell<Vec<SnapshotSummary>>,
    selected_snapshot_id: RefCell<Option<SnapshotId>>,
}

impl BlinkUi {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let data_dir = paths.app_data_dir(OrbitalApp::Blink);
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

        let header_bar = adw::HeaderBar::builder()
            .title_widget(&header_title)
            .build();
        header_bar.pack_start(&new_button);
        header_bar.pack_start(&import_button);

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
                "Blink now stores snapshot entries through orbital-core. \
This is the first persistent Phase 1 step before capture tooling lands.",
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

        let detail_title = gtk::Label::builder()
            .label("Blink")
            .xalign(0.0)
            .build();
        detail_title.add_css_class("title-1");

        let detail_body = gtk::Label::builder()
            .label("Create the first snapshot entry to start the local library.")
            .wrap(true)
            .xalign(0.0)
            .build();

        let preview_image = gtk::Image::new();
        preview_image.set_hexpand(true);
        preview_image.set_vexpand(true);

        let preview_placeholder = gtk::Label::builder()
            .label("Import an image to preview it here.")
            .wrap(true)
            .xalign(0.0)
            .build();
        preview_placeholder.add_css_class("dim-label");

        let preview_frame = gtk::Frame::new(None);
        preview_frame.set_size_request(520, 280);
        preview_frame.add_css_class("card");

        let preview_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        preview_box.append(&preview_image);
        preview_box.append(&preview_placeholder);
        preview_frame.set_child(Some(&preview_box));

        let (kind_row, detail_kind) = build_info_row("Kind");
        let (source_row, detail_source) = build_info_row("Source");
        let (file_path_row, detail_file_path) = build_info_row("File Path");
        let (mime_type_row, detail_mime_type) = build_info_row("MIME Type");
        let (tags_row, detail_tags) = build_info_row("Tags");
        let (id_row, detail_id) = build_info_row("Snapshot ID");
        let (storage_row, detail_storage) = build_info_row("Data Directory");
        detail_storage.set_label(&data_dir.display().to_string());

        let status_label = gtk::Label::builder()
            .label("Ready")
            .xalign(0.0)
            .build();
        status_label.add_css_class("dim-label");

        let detail_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        detail_panel.append(&detail_title);
        detail_panel.append(&detail_body);
        detail_panel.append(&preview_frame);
        detail_panel.append(&kind_row);
        detail_panel.append(&source_row);
        detail_panel.append(&file_path_row);
        detail_panel.append(&mime_type_row);
        detail_panel.append(&tags_row);
        detail_panel.append(&id_row);
        detail_panel.append(&storage_row);
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
            preview_image,
            preview_placeholder,
            detail_title,
            detail_body,
            detail_kind,
            detail_source,
            detail_file_path,
            detail_mime_type,
            detail_tags,
            detail_id,
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

        connect_actions(&ui, &new_button, &import_button);
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

        let mut new_snapshot = NewSnapshot::new(snapshot_id.clone(), title, SnapshotKind::Image);
        new_snapshot.source = Some(source_path.display().to_string());
        new_snapshot.file_path = Some(target_path.display().to_string());
        new_snapshot.mime_type = infer_mime_type(source_path);

        let snapshot = self.repository().create(new_snapshot)?;
        self.selected_snapshot_id
            .replace(Some(snapshot.id.clone()));
        self.reload_snapshots(Some(snapshot.id.clone()))?;
        self.set_status("Image imported");
        Ok(())
    }

    fn load_snapshot_into_detail(self: &Rc<Self>, index: usize) {
        let Some(snapshot) = self.snapshots.borrow().get(index).cloned() else {
            self.show_empty_state();
            return;
        };

        self.selected_snapshot_id
            .replace(Some(snapshot.id.clone()));
        self.detail_title.set_label(&snapshot.title);
        self.detail_body.set_label(
            "This is a persistent Blink snapshot entry stored in the shared OrbitalOS database. Capture and media workflows will build on top of this Phase 1 base.",
        );
        self.detail_kind.set_label(snapshot.kind.label());
        self.detail_source
            .set_label(snapshot.source.as_deref().unwrap_or("No source yet"));
        self.detail_file_path
            .set_label(snapshot.file_path.as_deref().unwrap_or("No file imported yet"));
        self.detail_mime_type
            .set_label(snapshot.mime_type.as_deref().unwrap_or("Unknown"));
        let tags_label = if snapshot.tags.is_empty() {
            "No tags yet".to_string()
        } else {
            snapshot.tags.join(", ")
        };
        self.detail_tags.set_label(&tags_label);
        self.detail_id.set_label(snapshot.id.as_str());
        self.update_preview(snapshot.file_path.as_deref());
        self.set_status("Snapshot loaded");
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
        self.detail_title.set_label("No snapshots yet");
        self.detail_body.set_label(
            "Create the first snapshot entry to start Blink's local library. The next steps will build capture and annotation tools on top of this shared storage base.",
        );
        self.detail_kind.set_label("Image");
        self.detail_source.set_label("Blink");
        self.detail_file_path.set_label("No file imported yet");
        self.detail_mime_type.set_label("Unknown");
        self.detail_tags.set_label("No tags yet");
        self.detail_id.set_label("Not created yet");
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
            self.preview_image.set_from_file(Some(path));
            self.preview_image.set_visible(true);
            self.preview_placeholder.set_visible(false);
        } else {
            self.preview_image.clear();
            self.preview_image.set_visible(false);
            self.preview_placeholder.set_visible(true);
        }
    }
}

fn connect_actions(ui: &Rc<BlinkUi>, new_button: &gtk::Button, import_button: &gtk::Button) {
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
