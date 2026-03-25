use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    GnomeScreenshot,
    GrimSlurp,
}

impl CaptureBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::GnomeScreenshot => "gnome-screenshot",
            Self::GrimSlurp => "grim + slurp",
        }
    }
}

pub fn detect_backend() -> Option<CaptureBackend> {
    if command_exists("gnome-screenshot") {
        return Some(CaptureBackend::GnomeScreenshot);
    }

    if command_exists("grim") && command_exists("slurp") {
        return Some(CaptureBackend::GrimSlurp);
    }

    None
}

pub fn capture_region(destination: &Path) -> Result<CaptureBackend, String> {
    let Some(backend) = detect_backend() else {
        return Err(
            "No supported screenshot backend found. Install gnome-screenshot or grim + slurp."
                .to_string(),
        );
    };

    match backend {
        CaptureBackend::GnomeScreenshot => run_gnome_screenshot(destination)?,
        CaptureBackend::GrimSlurp => run_grim_slurp(destination)?,
    }

    Ok(backend)
}

fn run_gnome_screenshot(destination: &Path) -> Result<(), String> {
    let status = Command::new("gnome-screenshot")
        .arg("-a")
        .arg("-f")
        .arg(destination)
        .status()
        .map_err(|error| format!("Failed to launch gnome-screenshot: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("Screenshot capture was cancelled or failed.".to_string())
    }
}

fn run_grim_slurp(destination: &Path) -> Result<(), String> {
    let selection = Command::new("slurp")
        .output()
        .map_err(|error| format!("Failed to launch slurp: {error}"))?;

    if !selection.status.success() {
        return Err("Screenshot capture was cancelled or failed.".to_string());
    }

    let geometry = String::from_utf8_lossy(&selection.stdout).trim().to_string();
    if geometry.is_empty() {
        return Err("Screenshot capture was cancelled.".to_string());
    }

    let status = Command::new("grim")
        .arg("-g")
        .arg(geometry)
        .arg(destination)
        .status()
        .map_err(|error| format!("Failed to launch grim: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("Screenshot capture was cancelled or failed.".to_string())
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
