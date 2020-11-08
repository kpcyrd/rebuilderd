use rebuilderd_common::errors::*;
use tokio::process::Command;

pub async fn diffoscope(a: &str, b: &str) -> Result<String> {
    let output = Command::new("diffoscope")
        .args(&["--", a, b])
        .output()
        .await
        .context("Failed to start diffoscope")?;
    info!("diffoscope exited with exit={}", output.status);
    let output = String::from_utf8_lossy(&output.stdout);
    Ok(output.into_owned())
}
