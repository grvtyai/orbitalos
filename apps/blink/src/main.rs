use std::cell::RefCell;
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
    list_box: gtk::ListBox,
    detail_title: gtk::Label,
    detail_body: gtk::Label,
    detail_kind: gtk::Label,
    detail_source: gtk::Label,
    detail_tags: gtk::Label,
    detail_id: gtk::Label,
    detail_storage: gtk::Label,
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

        let header_bar = adw::HeaderBar::builder()
            .title_widget(&header_title)
            .build();
        header_bar.pack_start(&new_button);

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

        let sidebar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .width_request(320)
            .build();
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

        let (kind_row, detail_kind) = build_info_row("Kind");
        let (source_row, detail_source) = build_info_row("Source");
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
        detail_panel.append(&kind_row);
        detail_panel.append(&source_row);
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

        let detail_frame = gtk::Frame::new(None);
        detail_frame.set_child(Some(&detail_panel));
        detail_frame.add_css_class("card");

        layout.append(&sidebar_frame);
        layout.append(&detail_frame);
        content.append(&layout);

        let ui = Rc::new(Self {
            database,
            list_box,
            detail_title,
            detail_body,
            detail_kind,
            detail_source,
            detail_tags,
            detail_id,
            detail_storage,
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

        connect_actions(&ui, &new_button);
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
            self.list_box.append(&build_snapshot_row(snapshot));
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
        let tags_label = if snapshot.tags.is_empty() {
            "No tags yet".to_string()
        } else {
            snapshot.tags.join(", ")
        };
        self.detail_tags.set_label(&tags_label);
        self.detail_id.set_label(snapshot.id.as_str());
        self.set_status("Snapshot loaded");
    }

    fn show_empty_state(&self) {
        self.selected_snapshot_id.replace(None);
        self.detail_title.set_label("No snapshots yet");
        self.detail_body.set_label(
            "Create the first snapshot entry to start Blink's local library. The next steps will build capture and annotation tools on top of this shared storage base.",
        );
        self.detail_kind.set_label("Image");
        self.detail_source.set_label("Blink");
        self.detail_tags.set_label("No tags yet");
        self.detail_id.set_label("Not created yet");
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
}

fn connect_actions(ui: &Rc<BlinkUi>, new_button: &gtk::Button) {
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

fn build_snapshot_row(snapshot: &SnapshotSummary) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(true);
    row.set_activatable(true);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();

    let title = gtk::Label::builder()
        .label(&snapshot.title)
        .xalign(0.0)
        .build();
    title.add_css_class("heading");

    let subtitle = gtk::Label::builder()
        .label(match snapshot.source.as_deref() {
            Some(source) => format!("{} - {source}", snapshot.kind.label()),
            None => snapshot.kind.label().to_string(),
        })
        .xalign(0.0)
        .wrap(true)
        .build();
    subtitle.add_css_class("dim-label");

    content.append(&title);
    content.append(&subtitle);
    row.set_child(Some(&content));
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
