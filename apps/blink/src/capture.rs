use std::path::PathBuf;

use ashpd::desktop::screenshot::Screenshot;

pub async fn capture_interactive() -> Result<PathBuf, String> {
    let response = Screenshot::request()
        .modal(true)
        .interactive(true)
        .send()
        .await
        .map_err(|error| format!("Screenshot portal request failed: {error}"))?
        .response()
        .map_err(|error| format!("Screenshot portal response failed: {error}"))?;

    response
        .uri()
        .to_file_path()
        .map_err(|_| "Screenshot portal returned a non-file URI.".to_string())
}
