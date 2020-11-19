use crate::config;
use crate::diffoscope::diffoscope;
use crate::download::download;
use futures::select;
use futures_util::FutureExt;
use rebuilderd_common::Distro;
use rebuilderd_common::api::{Rebuild, BuildStatus};
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use std::borrow::Cow;
use std::process::Stdio;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Context<'a> {
    pub script_location: Option<&'a PathBuf>,
    pub gen_diffoscope: bool,
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

    bail!("Failed to find a rebuilder script")
}

pub async fn rebuild<'a>(distro: &Distro, ctx: &Context<'a>, url: &str) -> Result<Rebuild> {
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let (input, filename) = download(url, &tmp)
        .await
        .context("Failed to download original package")?;

    let script = if let Some(script) = ctx.script_location {
        Cow::Borrowed(script)
    } else {
        let script = locate_script(distro, None)
            .context("Failed to locate rebuild script")?;
        Cow::Owned(script)
    };

    let (success, log) = verify(&script, &url.to_string(), &input).await?;

    if success {
        info!("rebuilder script indicated success");
        Ok(Rebuild::new(BuildStatus::Good, log))
    } else {
        info!("rebuilder script indicated error");
        let mut res = Rebuild::new(BuildStatus::Bad, log);

        // generate diffoscope diff if enabled
        if ctx.gen_diffoscope {
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
async fn verify(bin: &Path, url: &str, path: &str) -> Result<(bool, Vec<u8>)> {
    // TODO: establish a common interface to interface with distro rebuilders
    info!("executing rebuilder script at {:?}", bin);
    let mut child = Command::new(&bin)
        .args(&[url, path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let mut buf_stdout = [0u8; 4096];
    let mut buf_stderr = [0u8; 4096];

    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();

    let mut log = Vec::new();
    loop {
        select! {
            n = child_stdout.read(&mut buf_stdout).fuse() => {
                let n = n?;
                log.extend(&buf_stdout[..n]);
                stdout.write(&buf_stdout[..n]).await?;
            },
            n = child_stderr.read(&mut buf_stderr).fuse() => {
                let n = n?;
                log.extend(&buf_stderr[..n]);
                stderr.write(&buf_stderr[..n]).await?;
            },
            status = child.wait().fuse() => {
                let status = status?;
                info!("rebuilder script finished: {:?} (for {:?}, {:?})", status, url, path);
                return Ok((status.success(), log));
            }
        }
    }
}
