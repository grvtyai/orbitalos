use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::OrbitalApp;
use crate::error::{OrbitalError, OrbitalResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrbitalPaths {
    data_root: PathBuf,
    config_root: PathBuf,
    cache_root: PathBuf,
    documents_root: PathBuf,
}

impl OrbitalPaths {
    pub fn discover() -> OrbitalResult<Self> {
        let home = home_dir()?;

        let data_root = env_path("XDG_DATA_HOME")
            .unwrap_or_else(|| home.join(".local").join("share"))
            .join("orbitalos");

        let config_root = env_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home.join(".config"))
            .join("orbitalos");

        let cache_root = env_path("XDG_CACHE_HOME")
            .unwrap_or_else(|| home.join(".cache"))
            .join("orbitalos");

        let documents_root = home.join("Documents").join("OrbitalOS");

        Ok(Self {
            data_root,
            config_root,
            cache_root,
            documents_root,
        })
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    pub fn documents_root(&self) -> &Path {
        &self.documents_root
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_root.join("orbital.db")
    }

    pub fn attachments_dir(&self) -> PathBuf {
        self.documents_root.join("attachments")
    }

    pub fn app_data_dir(&self, app: OrbitalApp) -> PathBuf {
        self.data_root.join(app.slug())
    }

    pub fn app_config_dir(&self, app: OrbitalApp) -> PathBuf {
        self.config_root.join(app.slug())
    }

    pub fn app_cache_dir(&self, app: OrbitalApp) -> PathBuf {
        self.cache_root.join(app.slug())
    }

    pub fn app_documents_dir(&self, app: OrbitalApp) -> PathBuf {
        self.documents_root.join(app.display_name())
    }

    pub fn create_missing(&self) -> OrbitalResult<()> {
        fs::create_dir_all(&self.data_root)?;
        fs::create_dir_all(&self.config_root)?;
        fs::create_dir_all(&self.cache_root)?;
        fs::create_dir_all(&self.documents_root)?;
        fs::create_dir_all(self.attachments_dir())?;

        Ok(())
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key).map(PathBuf::from)
}

fn home_dir() -> OrbitalResult<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(OrbitalError::MissingHomeDirectory)
}

