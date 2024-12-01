use crate::config;
use crate::diffoscope::diffoscope;
use crate::download::download;
use crate::heartbeat::HeartBeat;
use crate::proc;
use in_toto::crypto::PrivateKey;
use in_toto::runlib::in_toto_run;
use rebuilderd_common::PkgArtifact;
use rebuilderd_common::api::{BuildStatus, Rebuild};
use rebuilderd_common::errors::Context as _;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::time;

pub struct Context<'a> {
    pub artifacts: Vec<PkgArtifact>,
    pub input_url: Option<String>,
    pub backend: config::Backend,
    pub build: config::Build,
    pub diffoscope: config::Diffoscope,
    pub privkey: &'a PrivateKey,
}

fn path_to_string(path: &Path) -> Result<String> {
    let s = path.to_str()
        .with_context(|| anyhow!("Path contains invalid characters: {:?}", path))?;
    Ok(s.to_string())
}

pub async fn compare_files(a: &Path, b: &Path) -> Result<bool> {
    let mut buf1 = [0u8; 4096];
    let mut buf2 = [0u8; 4096];

    info!("Comparing {:?} with {:?}", a, b);
    let mut f1 = File::open(a).await
        .with_context(|| anyhow!("Failed to open {:?}", a))?;
    let mut f2 = File::open(b).await
        .with_context(|| anyhow!("Failed to open {:?}", b))?;

    let mut pos = 0;
    loop {
        // read up to 4k bytes from the first file
        let n = f1.read_buf(&mut &mut buf1[..]).await?;

        // check if the first file is end-of-file
        if n == 0 {
            debug!("First file is at end-of-file");

            // check if other file is eof too
            let n = f2.read_buf(&mut &mut buf2[..]).await?;
            if n > 0 {
                info!("Files are not identical, {:?} is longer", b);
                return Ok(false);
            } else {
                return Ok(true);
            }
        }

        // check the same chunk in the other file
        match f2.read_exact(&mut buf2[..n]).await {
            Ok(n) => n,
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                info!("Files are not identical, {:?} is shorter", b);
                return Ok(false);
            },
            err => err?,
        };

        if buf1[..n] != buf2[..n] {
            // get the exact position
            // this can't panic because we've already checked the slices are not equal
            let pos = pos + buf1[..n].iter().zip(
                buf2[..n].iter()
            ).position(|(a,b)|a != b).unwrap();
            info!("Files {:?} and {:?} differ at position {}", a, b, pos);

            return Ok(false);
        }

        // advance the number of bytes that are equal
        pos += n;
    }
}

pub async fn rebuild_with_heartbeat(ctx: &Context<'_>, log: &mut Vec<u8>, hb: &dyn HeartBeat) -> Result<Vec<(PkgArtifact, Rebuild)>> {
    let mut rebuild = Box::pin(rebuild(ctx, log));
    loop {
        select! {
            res = &mut rebuild => {
                return res;
            },
            _ = time::sleep(hb.interval()) => hb.ping().await?,
        }
    }
}

pub async fn rebuild(ctx: &Context<'_>, log: &mut Vec<u8>,) -> Result<Vec<(PkgArtifact, Rebuild)>> {
    // setup
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let inputs_dir = tmp.path().join("inputs");
    fs::create_dir(&inputs_dir)
        .context("Failed to create inputs/ temp dir")?;

    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir)
        .context("Failed to create out/ temp dir")?;

    // download
    let mut artifacts = Vec::new();
    for artifact in &ctx.artifacts {
        let artifact_filename = download(&artifact.url, &inputs_dir)
            .await
            .with_context(|| anyhow!("Failed to download original package from {:?}", artifact.url))?;
        let artifact_path = inputs_dir.join(&artifact_filename);
        artifacts.push((artifact.clone(), artifact_filename, artifact_path));
    }

    let input_filename = if let Some(input_url) = &ctx.input_url {
        download(input_url, &inputs_dir)
            .await
            .with_context(|| anyhow!("Failed to download build input from {:?}", input_url))?
    } else {
        artifacts.first()
            .context("Failed to use first artifact as build input")?
            .1.to_owned()
    };
    let input_path = inputs_dir.join(&input_filename);

    // rebuild
    verify(ctx, log, &out_dir, &input_path).await?;

    // process results
    let mut results = Vec::new();
    for (artifact, artifact_filename, artifact_path) in artifacts {
        let output_path = out_dir.join(&artifact_filename);

        let result = if !output_path.exists() {
            info!("No output artifact found, marking as BAD: {:?}", output_path);
            Rebuild::new(BuildStatus::Bad)
        } else if compare_files(&artifact_path, &output_path).await? {
            info!("Output artifacts is identical, marking as GOOD: {:?}", output_path);
            let mut res = Rebuild::new(BuildStatus::Good);

            info!("Generating signed link");
            match in_toto_run(
                &format!("rebuild {}", artifact_filename.to_str().unwrap()),
                None,
                &[input_path.to_str().ok_or_else(|| anyhow!("Input path contains invalid characters"))?],
                &[output_path.to_str().ok_or_else(|| anyhow!("Output path contains invalid characters"))?],
                &[],
                Some(ctx.privkey),
                Some(&["sha512", "sha256"]),
                Some(&[
                    &format!("{}/", inputs_dir.to_str().unwrap()),
                    &format!("{}/", out_dir.to_str().unwrap()),
                ]),
            ) {
                Ok(signed_link) => {
                    info!("Signed link generated");

                    let attestation = serde_json::to_string(&signed_link).context("Failed to serialize attestation")?;
                    res.attestation = Some(attestation);
                }
                Err(err) => warn!("Failed to generate in-toto attestation: {:#?}", err),
            }

            res
        } else {
            info!("Output artifact differs, marking as BAD: {:?}", output_path);
            let mut res = Rebuild::new(BuildStatus::Bad);

            // generate diffoscope diff if enabled
            if ctx.diffoscope.enabled {
                let diff = diffoscope(&artifact_path, &output_path, &ctx.diffoscope)
                    .await
                    .context("Failed to run diffoscope")?;
                res.diffoscope = Some(diff);
            }

            res
        };

        results.push((artifact, result));
    }

    Ok(results)
}

async fn verify(ctx: &Context<'_>, log: &mut Vec<u8>, out_dir: &Path, input_path: &Path) -> Result<()> {
    let bin = &ctx.backend.path;
    let timeout = ctx.build.timeout.unwrap_or(3600 * 24); // 24h

    let mut envs = HashMap::new();
    envs.insert("REBUILDERD_OUTDIR".into(), path_to_string(out_dir)?);

    let opts = proc::Options {
        timeout: Duration::from_secs(timeout),
        size_limit: ctx.build.max_bytes,
        kill_at_size_limit: false,
        passthrough: !ctx.build.silent,
        envs,
    };

    proc::run(bin.as_ref(), &[input_path], opts, log).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compare_files_equal() {
        let equal = compare_files(Path::new("src/main.rs"), Path::new("src/main.rs")).await.unwrap();
        assert!(equal);
    }

    #[tokio::test]
    async fn compare_files_not_equal1() {
        let equal = compare_files(Path::new("src/main.rs"), Path::new("Cargo.toml")).await.unwrap();
        assert!(!equal);
    }

    #[tokio::test]
    async fn compare_files_not_equal2() {
        let equal = compare_files(Path::new("Cargo.toml"), Path::new("src/main.rs")).await.unwrap();
        assert!(!equal);
    }

    #[tokio::test]
    async fn compare_large_files_equal() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), [0u8; 4096 * 100]).unwrap();
        fs::write(dir.path().join("b"), [0u8; 4096 * 100]).unwrap();
        let equal = compare_files(&dir.path().join("a"), &dir.path().join("b")).await.unwrap();
        assert!(equal);
    }

    #[tokio::test]
    async fn compare_large_files_not_equal() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), [0u8; 4096 * 100]).unwrap();
        fs::write(dir.path().join("b"), [1u8; 4096 * 100]).unwrap();
        let equal = compare_files(&dir.path().join("a"), &dir.path().join("b")).await.unwrap();
        assert!(!equal);
    }
}
