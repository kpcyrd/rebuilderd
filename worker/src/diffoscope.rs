use rebuilderd_common::errors::*;
use tokio::process::Command;

pub async fn diffoscope(a: &str, b: &str) -> Result<String> {
    let output = Command::new("diffoscope")
        .args(&["--", a, b])
        .output()
        .await?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stdout);
        bail!("diffoscope exited with error: {:?}", err.trim());
    }
    let output = String::from_utf8(output.stdout)?;
    Ok(output)
}
