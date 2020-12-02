use crate::config;
use crate::proc;
use crate::diffoscope::diffoscope;
use crate::download::download;
use rebuilderd_common::Distro;
use rebuilderd_common::api::{Rebuild, BuildStatus};
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Default)]
pub struct Context<'a> {
    pub script_location: Option<&'a PathBuf>,
    pub diffoscope: config::Diffoscope,
}

fn locate_script(distro: &Distro, script_location: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(script_location) = script_location {
        return Ok(script_location);
    }

    let bin = match distro {
        Distro::Archlinux => "rebuilder-archlinux.sh",
        Distro::Debian => "rebuilder-debian.sh",
    };

    for prefix in &[".", "/usr/libexec/rebuilderd", "/usr/local/libexec/rebuilderd"] {
        let bin = format!("{}/{}", prefix, bin);
        let bin = Path::new(&bin);

        if bin.exists() {
            return Ok(bin.to_path_buf());
        }
    }

    bail!("Failed to find a rebuilder backend")
}

pub async fn rebuild<'a>(distro: &Distro, ctx: &Context<'a>, url: &str) -> Result<Rebuild> {
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let (input, filename) = download(url, &tmp)
        .await
        .with_context(|| anyhow!("Failed to download original package from {:?}", url))?;

    let script = if let Some(script) = ctx.script_location {
        Cow::Borrowed(script)
    } else {
        let script = locate_script(distro, None)
            .with_context(|| anyhow!("Failed to locate rebuild backend for distro={}", distro))?;
        Cow::Owned(script)
    };

    let (success, log) = verify(&script, &url.to_string(), &input).await?;

    if success {
        info!("Rebuilder backend indicated a success rebuild!");
        Ok(Rebuild::new(BuildStatus::Good, log))
    } else {
        info!("Rebuilder backend exited with non-zero exit code");
        let mut res = Rebuild::new(BuildStatus::Bad, log);

        // generate diffoscope diff if enabled
        if ctx.diffoscope.enabled {
            let output = Path::new("./build/").join(filename);
            if output.exists() {
                let output = output.to_str()
                    .ok_or_else(|| format_err!("Output path contains invalid characters"))?;

                let diff = diffoscope(&input, output, &ctx.diffoscope)
                    .await
                    .context("Failed to run diffoscope")?;
                res.diffoscope = Some(diff);
            } else {
                info!("Skipping diffoscope because rebuilder script did not produce output");
            }
        }

        Ok(res)
    }
}

// TODO: automatically truncate logs to a max-length if configured
async fn verify(bin: &Path, url: &str, path: &str) -> Result<(bool, String)> {
    // TODO: establish a common interface to interface with distro rebuilders
    let args = &[url, path];

    let timeout = 3600 * 24; // 24h

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout),
        limit: None,
        kill_at_size_limit: false,
    };
    proc::run(bin, args, opts).await
}
