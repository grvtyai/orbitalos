use std::io;
use std::path::Path;
use std::process::Command;

pub fn capture_interactive(target_path: &Path) -> Result<(), String> {
    match capture_with_gnome_screenshot(target_path) {
        Ok(()) => Ok(()),
        Err(CaptureError::BackendUnavailable(_)) => capture_with_grim(target_path).map_err(|error| {
            format!(
                "{error}. Install `gnome-screenshot` or `grim` + `slurp` to enable Blink capture."
            )
        }),
        Err(error) => Err(error.to_string()),
    }
}

fn capture_with_gnome_screenshot(target_path: &Path) -> Result<(), CaptureError> {
    let status = Command::new("gnome-screenshot")
        .arg("-a")
        .arg("-f")
        .arg(target_path)
        .status()
        .map_err(|error| classify_backend_error("gnome-screenshot", error))?;

    if status.success() {
        return Ok(());
    }

    Err(CaptureError::Failed(format!(
        "gnome-screenshot exited with status {status}"
    )))
}

fn capture_with_grim(target_path: &Path) -> Result<(), CaptureError> {
    let selection = Command::new("slurp")
        .output()
        .map_err(|error| classify_backend_error("slurp", error))?;

    if !selection.status.success() {
        return Err(CaptureError::Failed(format!(
            "slurp exited with status {}",
            selection.status
        )));
    }

    let geometry = String::from_utf8(selection.stdout)
        .map_err(|error| CaptureError::Failed(format!("slurp returned invalid UTF-8: {error}")))?;
    let geometry = geometry.trim();

    if geometry.is_empty() {
        return Err(CaptureError::Failed(
            "No screen area was selected for capture".to_string(),
        ));
    }

    let status = Command::new("grim")
        .arg("-g")
        .arg(geometry)
        .arg(target_path)
        .status()
        .map_err(|error| classify_backend_error("grim", error))?;

    if status.success() {
        return Ok(());
    }

    Err(CaptureError::Failed(format!(
        "grim exited with status {status}"
    )))
}

fn classify_backend_error(command: &'static str, error: io::Error) -> CaptureError {
    if error.kind() == io::ErrorKind::NotFound {
        CaptureError::BackendUnavailable(command)
    } else {
        CaptureError::Failed(format!("{command} could not start: {error}"))
    }
}

enum CaptureError {
    BackendUnavailable(&'static str),
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable(command) => {
                write!(f, "Capture backend `{command}` is not available")
            }
            Self::Failed(message) => f.write_str(message),
        }
    }
}
