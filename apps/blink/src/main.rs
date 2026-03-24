use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

use orbital_core::{OrbitalApp, OrbitalPaths};

fn main() {
    adw::init().expect("Failed to initialize Libadwaita");

    let app = adw::Application::builder()
        .application_id(OrbitalApp::Blink.application_id())
        .build();

    app.connect_activate(|app| match BlinkWindow::build(app) {
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

struct BlinkWindow;

impl BlinkWindow {
    fn build(app: &adw::Application) -> orbital_core::OrbitalResult<adw::ApplicationWindow> {
        let paths = OrbitalPaths::discover()?;
        let app_descriptor = OrbitalApp::Blink.descriptor();
        let config_dir = paths.app_config_dir(OrbitalApp::Blink);
        let data_dir = paths.app_data_dir(OrbitalApp::Blink);
        let cache_dir = paths.app_cache_dir(OrbitalApp::Blink);

        let header_title = adw::WindowTitle::builder()
            .title(app_descriptor.display_name)
            .subtitle("Snapshot workspace scaffold")
            .build();

        let header_bar = adw::HeaderBar::builder()
            .title_widget(&header_title)
            .show_title(true)
            .build();

        let intro_title = gtk::Label::builder()
            .label("Blink")
            .xalign(0.0)
            .build();
        intro_title.add_css_class("title-1");

        let intro_body = gtk::Label::builder()
            .label(
                "Blink is being prepared as the OrbitalOS snapshot app. \
It already uses the shared OrbitalOS application identity and path layout so \
its storage and sync foundations can stay aligned with Drift and future apps.",
            )
            .wrap(true)
            .xalign(0.0)
            .build();
        intro_body.add_css_class("body");

        let app_id_row = build_info_row("Application ID", &app_descriptor.application_id);
        let slug_row = build_info_row("App Slug", app_descriptor.slug);
        let config_row = build_info_row("Config Directory", &config_dir.display().to_string());
        let data_row = build_info_row("Data Directory", &data_dir.display().to_string());
        let cache_row = build_info_row("Cache Directory", &cache_dir.display().to_string());

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        info_box.append(&app_id_row);
        info_box.append(&slug_row);
        info_box.append(&config_row);
        info_box.append(&data_row);
        info_box.append(&cache_row);

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        card.add_css_class("card");
        card.append(&intro_title);
        card.append(&intro_body);
        card.append(&info_box);

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&header_bar);
        content.append(&card);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(app_descriptor.display_name)
            .default_width(960)
            .default_height(640)
            .content(&content)
            .build();

        Ok(window)
    }
}

fn build_info_row(label: &str, value: &str) -> gtk::Box {
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
        .label(value)
        .wrap(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    row_value.add_css_class("dim-label");

    row.append(&row_label);
    row.append(&row_value);
    row
}
