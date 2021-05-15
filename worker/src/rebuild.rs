use crate::config;
use crate::diffoscope::diffoscope;
use crate::download::download;
use crate::proc;
use rebuilderd_common::Distro;
use rebuilderd_common::api::{Rebuild, BuildStatus};
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
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

fn path_to_string(path: &Path) -> Result<&str> {
    let s = path.to_str()
        .with_context(|| anyhow!("Path contains invalid characters: {:?}", path))?;
    Ok(s)
}

pub async fn rebuild(ctx: &Context<'_>, url: &str) -> Result<Rebuild> {
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let inputs_dir = tmp.path().join("inputs");
    fs::create_dir(&inputs_dir)
        .context("Failed to create inputs/ temp dir")?;

    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir)
        .context("Failed to create out/ temp dir")?;

    let (pkg_path, filename) = download(url, &inputs_dir)
        .await
        .with_context(|| anyhow!("Failed to download original package from {:?}", url))?;

    let (success, log) = verify(ctx, &out_dir, &pkg_path).await?;

    if success {
        info!("Rebuilder backend indicated a success rebuild!");
        Ok(Rebuild::new(BuildStatus::Good, log))
    } else {
        info!("Rebuilder backend exited with non-zero exit code");
        let mut res = Rebuild::new(BuildStatus::Bad, log);

        // generate diffoscope diff if enabled
        if ctx.diffoscope.enabled {
            let output = out_dir.join(filename);
            if output.exists() {
                let diff = diffoscope(&pkg_path, path_to_string(&output)?, &ctx.diffoscope)
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

async fn verify(ctx: &Context<'_>, out_dir: &Path, pkg_path: &str) -> Result<(bool, String)> {
    let bin = if let Some(script) = ctx.script_location {
        Cow::Borrowed(script)
    } else {
        let script = locate_script(ctx.distro, None)
            .with_context(|| anyhow!("Failed to locate rebuild backend for distro={}", ctx.distro))?;
        Cow::Owned(script)
    };

    let timeout = ctx.build.timeout.unwrap_or(3600 * 24); // 24h

    let mut envs = HashMap::new();
    envs.insert("REBUILDERD_OUTDIR".into(), path_to_string(out_dir)?.into());

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout),
        size_limit: ctx.build.max_bytes,
        kill_at_size_limit: false,
        passthrough: !ctx.build.silent,
        envs,
    };
    proc::run(bin.as_ref(), &[pkg_path], opts).await
}
