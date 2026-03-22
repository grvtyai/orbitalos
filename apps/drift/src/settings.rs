use std::fs;
use std::path::PathBuf;

use orbital_core::{OrbitalApp, OrbitalPaths};

use crate::block_layout;

const SETTINGS_FILE_NAME: &str = "settings.conf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDensity {
    ExtraFine,
    Fine,
    Standard,
    Relaxed,
    Wide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftSettings {
    pub grid_density: GridDensity,
}

impl Default for DriftSettings {
    fn default() -> Self {
        Self {
            grid_density: GridDensity::Standard,
        }
    }
}

impl DriftSettings {
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

            if key.trim() == "grid_density" {
                settings.grid_density = GridDensity::from_storage(value.trim());
            }
        }

        settings
    }

    pub fn save(&self, paths: &OrbitalPaths) -> Result<(), String> {
        let config_dir = paths.app_config_dir(OrbitalApp::Drift);
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;

        let contents = format!(
            "# Drift settings\n# Stored in OrbitalOS app config\n\ngrid_density={}\n",
            self.grid_density.storage_key()
        );

        fs::write(config_dir.join(SETTINGS_FILE_NAME), contents)
            .map_err(|error| error.to_string())
    }

    pub fn grid_size(&self) -> i32 {
        self.grid_density.grid_size()
    }
}

impl GridDensity {
    pub fn from_index(index: u32) -> Self {
        match index {
            0 => Self::ExtraFine,
            1 => Self::Fine,
            2 => Self::Standard,
            3 => Self::Relaxed,
            4 => Self::Wide,
            _ => Self::Standard,
        }
    }

    pub fn index(self) -> u32 {
        match self {
            Self::ExtraFine => 0,
            Self::Fine => 1,
            Self::Standard => 2,
            Self::Relaxed => 3,
            Self::Wide => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ExtraFine => "Sehr fein",
            Self::Fine => "Fein",
            Self::Standard => "Standard",
            Self::Relaxed => "Locker",
            Self::Wide => "Groß",
        }
    }

    pub fn grid_size(self) -> i32 {
        match self {
            Self::ExtraFine => 4,
            Self::Fine => 6,
            Self::Standard => block_layout::DEFAULT_GRID_SIZE,
            Self::Relaxed => 12,
            Self::Wide => 16,
        }
    }

    fn from_storage(value: &str) -> Self {
        match value {
            "extra-fine" => Self::ExtraFine,
            "fine" => Self::Fine,
            "standard" => Self::Standard,
            "relaxed" => Self::Relaxed,
            "wide" => Self::Wide,
            _ => Self::Standard,
        }
    }

    fn storage_key(self) -> &'static str {
        match self {
            Self::ExtraFine => "extra-fine",
            Self::Fine => "fine",
            Self::Standard => "standard",
            Self::Relaxed => "relaxed",
            Self::Wide => "wide",
        }
    }
}

pub fn grid_density_labels() -> [&'static str; 5] {
    [
        GridDensity::ExtraFine.label(),
        GridDensity::Fine.label(),
        GridDensity::Standard.label(),
        GridDensity::Relaxed.label(),
        GridDensity::Wide.label(),
    ]
}

fn settings_path(paths: &OrbitalPaths) -> PathBuf {
    paths.app_config_dir(OrbitalApp::Drift).join(SETTINGS_FILE_NAME)
}
