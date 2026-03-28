use std::fs;
use std::path::PathBuf;

use orbital_core::{OrbitalApp, OrbitalPaths};

const SETTINGS_FILE_NAME: &str = "settings.conf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlinkSettings {
    pub theme_mode: ThemeMode,
}

impl Default for BlinkSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
        }
    }
}

impl BlinkSettings {
    pub fn load(paths: &OrbitalPaths) -> Self {
        let path = settings_path(paths);
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };

        let mut settings = Self::default();

        for line in contents.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };

            if key.trim() == "theme_mode" {
                settings.theme_mode = ThemeMode::from_storage(value.trim());
            }
        }

        settings
    }

    pub fn save(&self, paths: &OrbitalPaths) -> Result<(), String> {
        let config_dir = paths.app_config_dir(OrbitalApp::Blink);
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;

        let contents = format!(
            "# Blink settings\n# Stored in OrbitalOS app config\n\ntheme_mode={}\n",
            self.theme_mode.storage_key()
        );

        fs::write(config_dir.join(SETTINGS_FILE_NAME), contents)
            .map_err(|error| error.to_string())
    }
}

impl ThemeMode {
    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Light,
            _ => Self::Dark,
        }
    }

    pub fn index(self) -> u32 {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    fn from_storage(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            _ => Self::Dark,
        }
    }

    fn storage_key(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

pub fn theme_mode_labels() -> [&'static str; 2] {
    [ThemeMode::Dark.label(), ThemeMode::Light.label()]
}

fn settings_path(paths: &OrbitalPaths) -> PathBuf {
    paths.app_config_dir(OrbitalApp::Blink).join(SETTINGS_FILE_NAME)
}
