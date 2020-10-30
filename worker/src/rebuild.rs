use rebuilderd_common::Distro;
use rebuilderd_common::api::{Rebuild, BuildStatus};
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::fs::File;
use std::process::Command;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Default)]
pub struct Context<'a> {
    pub script_location: Option<&'a PathBuf>,
    pub gen_diffoscope: bool,
}

pub fn rebuild(distro: &Distro, ctx: &Context, url: &str) -> Result<Rebuild> {
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let url = url.parse::<Url>()
        .context("Failed to parse input as url")?;

    let filename = url.path_segments()
        .ok_or_else(|| format_err!("Url doesn't seem to have a path"))?
        .last()
        .ok_or_else(|| format_err!("Failed to get filename from path"))?;
    if filename.is_empty() {
        bail!("Filename is empty");
    }

    let input = tmp.path().join(filename);
    download(&url, &input)
        .context("Failed to download original package")?;
    let input = input.to_str()
        .ok_or_else(|| format_err!("Input path contains invalid characters"))?;

    if !verify(distro, ctx.script_location, &url.to_string(), input)? {
        Ok(Rebuild::new(BuildStatus::Bad))
    } else {
        info!("rebuilder script indicated success");
        let mut res = Rebuild::new(BuildStatus::Good);

        let output = Path::new("./build/").join(filename);
        if !output.exists() {
            bail!("Rebuild script exited successfully but output package does not exist");
        }
        let output = output.to_str()
            .ok_or_else(|| format_err!("Output path contains invalid characters"))?;

        // TODO: diff files. this is already done by the rebuilder script right now, but we'd rather do it here

        if ctx.gen_diffoscope {
            let diff = diffoscope(input, output)
                .context("Failed to run diffoscope")?;
            res.diffoscope = Some(diff);
        }

        Ok(res)
    }
}

fn download(url: &Url, target: &Path) -> Result<()> {
    info!("Downloading {:?} to {:?}", url, target);
    let client = reqwest::blocking::Client::new();
    let mut response = client.get(&url.to_string())
        .send()?
        .error_for_status()?;

    let mut f = File::create(target)
        .context("Failed to create output file")?;
    let n = response.copy_to(&mut f)
        .context("Failed to download")?;
    info!("Downloaded {} bytes", n);

    Ok(())
}

fn diffoscope(a: &str, b: &str) -> Result<String> {
    let output = Command::new("diffoscope")
        .args(&["--", a, b])
        .output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stdout);
        bail!("diffoscope exited with error: {:?}", err.trim());
    }
    let output = String::from_utf8(output.stdout)?;
    Ok(output)
}

fn verify(distro: &Distro, script_location: Option<&PathBuf>, url: &str, path: &str) -> Result<bool> {
    if let Some(script) = script_location {
        spawn_script(&script, url, path)
    } else {
        let script = locate_script(distro)?;
        spawn_script(&script, url, path)
    }
}

fn locate_script(distro: &Distro) -> Result<PathBuf> {
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

fn spawn_script(bin: &Path, url: &str, path: &str) -> Result<bool> {
    // TODO: establish a common interface to interface with distro rebuilders
    info!("executing rebuilder script at {:?}", bin);
    let status = Command::new(&bin)
        .args(&[url, path])
        .status()?;

    info!("rebuilder script finished: {:?} (for {:?}, {:?})", status, url, path);
    Ok(status.success())
}
