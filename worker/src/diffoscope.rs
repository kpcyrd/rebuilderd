use crate::config;
use crate::log;
use crate::proc;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

const DIFF_DEFAULT_TIMEOUT: u64 = 3600; // 1h
// Add a grace period to --timeout before sending SIGTERM (followed by sigkill_delay, then SIGKILL)
const SIGTERM_DELAY: u64 = 600;

pub async fn diffoscope(a: &Path, b: &Path, settings: &config::Diffoscope) -> Result<String> {
    let mut args = settings.args.iter().map(OsString::from).collect::<Vec<_>>();
    let timeout = settings.timeout.unwrap_or(DIFF_DEFAULT_TIMEOUT);

    let sigkill_delay = Duration::from_secs(settings.sigkill_delay);

    args.push(format!("--timeout={timeout}").into());
    args.push("--".into());
    args.push(a.into());
    args.push(b.into());

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout + SIGTERM_DELAY), // give diffoscope extra time to finish on it's own
        kill_at_size_limit: true,
        sigkill_delay,
        passthrough: false,
        envs: HashMap::new(),
    };
    let bin = Path::new("diffoscope");

    let mut output = log::Buffer::new(settings.max_bytes, Some(0));
    proc::run(bin, &args, opts, &mut output).await?;
    let output = output.make_string();

    Ok(output)
}
