use crate::config;
use crate::log;
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
        timeout: Duration::from_secs(timeout + 600), // give diffoscope 10 extra minutes to finish
        front_size_limit: settings.max_bytes,
        tail_size_limit: Some(0),
        kill_at_size_limit: true,
        passthrough: false,
        envs: HashMap::new(),
    };
    let bin = Path::new("diffoscope");

    let mut output = log::Buffer::from_opts(&opts);
    proc::run(bin, &args, opts, &mut output).await?;
    let output = output.make_string();

    Ok(output)
}
