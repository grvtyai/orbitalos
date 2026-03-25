use std::path::PathBuf;

use ashpd::desktop::screenshot::Screenshot;
use ashpd::url::Url;

pub async fn capture_interactive() -> Result<PathBuf, String> {
    let response = Screenshot::request()
        .modal(true)
        .interactive(true)
        .send()
        .await
        .map_err(|error| format!("Screenshot portal request failed: {error}"))?
        .response()
        .map_err(|error| format!("Screenshot portal response failed: {error}"))?;

    Url::parse(&response.uri().to_string())
        .map_err(|error| format!("Screenshot portal returned an invalid URI: {error}"))?
        .to_file_path()
        .map_err(|_| "Screenshot portal returned a non-file URI.".to_string())
}
