use crate::config;
use futures_util::FutureExt;
use rebuilderd_common::errors::*;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::{Command, Child};
use tokio::select;
use tokio::time;

pub struct Capture {
    output: Vec<u8>,
    timeout: Duration,
    limit: Option<usize>,
    start: Instant,
    closed: bool,
}

impl Capture {
    fn new(timeout: Duration, limit: Option<usize>) -> Capture {
        let start = Instant::now();
        Capture {
            output: Vec::new(),
            timeout,
            limit,
            start,
            closed: false,
        }
    }

    #[inline]
    fn into_output(self) -> Vec<u8> {
        self.output
    }

    async fn push_bytes(&mut self, child: &mut Child, slice: &[u8]) -> Result<()> {
        if let Some(limit) = &self.limit {
            if self.output.len() + slice.len() > *limit {
                if !self.closed {
                    warn!("Exceeding output limit: output={}, slice={}, limit={}", self.output.len(), slice.len(), limit);
                    self.truncate(child, "TRUNCATED DUE TO SIZE LIMIT").await?;
                }
                return Ok(());
            }
        }

        self.output.extend(slice);
        Ok(())
    }

    async fn truncate(&mut self, child: &mut Child, reason: &str) -> Result<()> {
        child.kill().await?;
        self.output.extend(format!("\n\n{}\n", reason).as_bytes());
        self.closed = true;
        Ok(())
    }

    async fn check_timeout(&mut self, child: &mut Child) -> Result<Duration> {
        if let Some(remaining) = self.timeout.checked_sub(self.start.elapsed()) {
            Ok(remaining)
        } else {
            if !self.closed {
                warn!("diffoscope timed out, killing...");
                self.truncate(child, "TRUNCATED DUE TO TIMEOUT").await?;
            }
            // we need to return a value to make our select! loop happy
            // the loop short terminate shortly after though
            Ok(Duration::from_secs(60))
        }
    }
}

pub async fn diffoscope(a: &str, b: &str, settings: &config::Diffoscope) -> Result<String> {
    let mut args = settings.args.clone();
    args.push("--".into());
    args.push(a.into());
    args.push(b.into());

    debug!("Running diffoscope {:?}", args);
    let mut child = Command::new("diffoscope")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let mut buf_stdout = [0u8; 4096];
    let mut buf_stderr = [0u8; 4096];

    let timeout = settings.timeout.unwrap_or(3600); // 1h
    let timeout = Duration::from_secs(timeout);

    let mut cap = Capture::new(timeout, settings.max_bytes);
    let output = loop {
        let remaining = cap.check_timeout(&mut child).await?;

        select! {
            n = child_stdout.read(&mut buf_stdout).fuse() => {
                let n = n?;
                cap.push_bytes(&mut child, &buf_stdout[..n]).await?;
            },
            n = child_stderr.read(&mut buf_stderr).fuse() => {
                let n = n?;
                cap.push_bytes(&mut child, &buf_stderr[..n]).await?;
            },
            status = child.wait().fuse() => {
                let status = status?;
                let output = cap.into_output();
                info!("diffoscope exited with exit={}, captured {} bytes", status, output.len());
                break output;
            }
            _ = time::sleep(remaining).fuse() => continue,
        }
    };

    let output = String::from_utf8_lossy(&output);
    Ok(output.into_owned())
}
