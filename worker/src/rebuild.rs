use crate::config;
use crate::diffoscope::diffoscope;
use crate::download::download;
use crate::proc;
use in_toto::crypto::PrivateKey;
use in_toto::runlib::in_toto_run;
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

pub struct Context<'a> {
    pub artifact_url: &'a str,
    pub input_url: Option<&'a str>,
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

pub async fn rebuild(ctx: &Context<'_>) -> Result<Rebuild> {
    // setup
    let tmp = tempfile::Builder::new().prefix("rebuilderd").tempdir()?;

    let inputs_dir = tmp.path().join("inputs");
    fs::create_dir(&inputs_dir)
        .context("Failed to create inputs/ temp dir")?;

    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir)
        .context("Failed to create out/ temp dir")?;

    // download
    let artifact_filename = download(ctx.artifact_url, &inputs_dir)
        .await
        .with_context(|| anyhow!("Failed to download original package from {:?}", ctx.artifact_url))?;
    let artifact_path = inputs_dir.join(&artifact_filename);

    let input_filename = if let Some(input_url) = &ctx.input_url {
        download(input_url, &inputs_dir)
            .await
            .with_context(|| anyhow!("Failed to download original package from {:?}", ctx.artifact_url))?
    } else {
        artifact_filename.to_owned()
    };
    let input_path = inputs_dir.join(&input_filename);

    // rebuild
    let log = verify(ctx, &out_dir, &input_path).await?;

    // process result
    let output_path = out_dir.join(&artifact_filename);

    if !output_path.exists() {
        info!("Build failed, no output artifact found at {:?}", output_path);
        Ok(Rebuild::new(BuildStatus::Bad, log))
    } else if compare_files(&artifact_path, &output_path).await? {
        info!("Files are identical, marking as GOOD");
        let mut res = Rebuild::new(BuildStatus::Good, log);

        info!("Generating signed link");
        let signed_link = in_toto_run(
            &format!("rebuild {}", artifact_filename.to_str().unwrap()),
            None,
            &[input_path.to_str().ok_or_else(|| anyhow!("Input path contains invalid characters"))?],
            &[output_path.to_str().ok_or_else(|| anyhow!("Output path contains invalid characters"))?],
            &[],
            Some(ctx.privkey),
            Some(&["sha512", "sha256"]),
        ).context("Failed to generate in-toto attestation")?;
        info!("Signed link generated");

        let attestation = serde_json::to_string(&signed_link).context("Failed to serialize attestation")?;
        res.attestation = Some(attestation);

        Ok(res)
    } else {
        info!("Build successful but artifacts differ");
        let mut res = Rebuild::new(BuildStatus::Bad, log);

        // generate diffoscope diff if enabled
        if ctx.diffoscope.enabled {
            let diff = diffoscope(&input_path, &output_path, &ctx.diffoscope)
                .await
                .context("Failed to run diffoscope")?;
            res.diffoscope = Some(diff);
        }

        Ok(res)
    }
}

async fn verify(ctx: &Context<'_>, out_dir: &Path, input_path: &Path) -> Result<String> {
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
    let (_success, log) = proc::run(bin.as_ref(), &[input_path], opts).await?;

    Ok(log)
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
        fs::write(dir.path().join("a"), &[0u8; 4096 * 100]).unwrap();
        fs::write(dir.path().join("b"), &[0u8; 4096 * 100]).unwrap();
        let equal = compare_files(&dir.path().join("a"), &dir.path().join("b")).await.unwrap();
        assert!(equal);
    }

    #[tokio::test]
    async fn compare_large_files_not_equal() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), &[0u8; 4096 * 100]).unwrap();
        fs::write(dir.path().join("b"), &[1u8; 4096 * 100]).unwrap();
        let equal = compare_files(&dir.path().join("a"), &dir.path().join("b")).await.unwrap();
        assert!(!equal);
    }
}
