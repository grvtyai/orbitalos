#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrbitalApp {
    Control,
    Dock,
    Drift,
    Prism,
    Relay,
    Vector,
    Vlink,
}

pub const APP_NAMESPACE: &str = "io.github.grvtyai.orbitalos";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDescriptor {
    pub app: OrbitalApp,
    pub display_name: &'static str,
    pub slug: &'static str,
    pub application_id: String,
}

impl OrbitalApp {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Dock => "dock",
            Self::Drift => "drift",
            Self::Prism => "prism",
            Self::Relay => "relay",
            Self::Vector => "vector",
            Self::Vlink => "vlink",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Control => "Control",
            Self::Dock => "Dock",
            Self::Drift => "Drift",
            Self::Prism => "Prism",
            Self::Relay => "Relay",
            Self::Vector => "Vector",
            Self::Vlink => "Vlink",
        }
    }

    pub fn application_id(self) -> String {
        format!("{APP_NAMESPACE}.{}", self.slug())
    }

    pub fn descriptor(self) -> AppDescriptor {
        AppDescriptor {
            app: self,
            display_name: self.display_name(),
            slug: self.slug(),
            application_id: self.application_id(),
        }
    }
}

