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

pub struct Context<'a> {
    pub distro: &'a Distro,
    pub script_location: Option<&'a PathBuf>,
    pub build: config::Build,
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

pub async fn rebuild<'a>(ctx: &Context<'a>, url: &str) -> Result<Rebuild> {
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let (input, filename) = download(url, &tmp)
        .await
        .with_context(|| anyhow!("Failed to download original package from {:?}", url))?;

    let (success, log) = verify(ctx, &input).await?;

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
async fn verify<'a>(ctx: &Context<'a>, path: &str) -> Result<(bool, String)> {
    let bin = if let Some(script) = ctx.script_location {
        Cow::Borrowed(script)
    } else {
        let script = locate_script(ctx.distro, None)
            .with_context(|| anyhow!("Failed to locate rebuild backend for distro={}", ctx.distro))?;
        Cow::Owned(script)
    };

    // TODO: establish a common interface to interface with distro rebuilders
    // TODO: specify the path twice because the 2nd argument used to be the path
    // TODO: we want to move this to the first instead. the 2nd argument can be removed in the future
    let args = &[path, path];

    let timeout = ctx.build.timeout.unwrap_or(3600 * 24); // 24h

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout),
        size_limit: ctx.build.max_bytes,
        kill_at_size_limit: false,
    };
    proc::run(bin.as_ref(), args, opts).await
}
