use crate::config;
use crate::proc;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

pub async fn diffoscope(a: &Path, b: &Path, settings: &config::Diffoscope) -> Result<String> {
    let mut args = settings.args.iter().map(OsString::from).collect::<Vec<_>>();
    let timeout = settings.timeout.unwrap_or(3600); // 1h

    args.push(format!("--timeout={timeout}").into());
    args.push("--".into());
    args.push(a.into());
    args.push(b.into());

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout + 600), // give diffoscope 10 minutes to finish
        size_limit: settings.max_bytes,
        kill_at_size_limit: true,
        passthrough: false,
        envs: HashMap::new(),
    };
    let bin = Path::new("diffoscope");

    let mut output = Vec::new();
    proc::run(bin, &args, opts, &mut output).await?;
    let output = String::from_utf8_lossy(&output);

    Ok(output.into_owned())
}
