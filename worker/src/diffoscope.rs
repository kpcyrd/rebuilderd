use crate::config;
use futures_util::FutureExt;
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use rebuilderd_common::errors::*;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::{Command, Child};
use tokio::select;
use tokio::time;

const SIGKILL_DELAY: u64 = 10;

pub struct Capture {
    output: Vec<u8>,
    timeout: Duration,
    limit: Option<usize>,
    start: Instant,
    closed: Option<Instant>,
}

impl Capture {
    fn new(timeout: Duration, limit: Option<usize>) -> Capture {
        let start = Instant::now();
        Capture {
            output: Vec::new(),
            timeout,
            limit,
            start,
            closed: None,
        }
    }

    #[inline]
    fn into_output(self) -> Vec<u8> {
        self.output
    }

    async fn push_bytes(&mut self, child: &mut Child, slice: &[u8]) -> Result<()> {
        if let Some(limit) = &self.limit {
            if self.output.len() + slice.len() > *limit {
                if self.closed.is_none() {
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
        if let Some(pid) = child.id() {
            info!("Sending SIGTERM to diffoscope(pid={})", pid);
            signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)?;
        }

        self.output.extend(format!("\n\n{}\n", reason).as_bytes());
        self.closed = Some(Instant::now());
        Ok(())
    }

    async fn next_wakeup(&mut self, child: &mut Child) -> Result<Duration> {
        // check if we need to SIGKILL
        if let Some(closed) = self.closed {
            if closed.elapsed() > Duration::from_secs(SIGKILL_DELAY) {
                if let Some(pid) = child.id() {
                    warn!("diffoscope(pid={}) didn't terminate {}s after SIGTERM, sending SIGKILL", pid, SIGKILL_DELAY);
                    // child.id is going to return None after this
                    child.kill().await?;
                }
            }
        }

        // check if the process timed out and we need to SIGTERM
        if let Some(remaining) = self.timeout.checked_sub(self.start.elapsed()) {
            return Ok(remaining);
        } else if self.closed.is_none() {
            // the process has timed out, sending SIGTERM
            warn!("diffoscope timed out, killing...");
            self.truncate(child, "TRUNCATED DUE TO TIMEOUT").await?;
        }

        // if we don't need any timeouts anymore we just return any value
        Ok(Duration::from_secs(SIGKILL_DELAY))
    }
}

pub async fn diffoscope(a: &str, b: &str, settings: &config::Diffoscope) -> Result<String> {
    let mut args = settings.args.clone();
    args.push("--".into());
    args.push(a.into());
    args.push(b.into());

    info!("Running diffoscope {:?}", args);
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
        let remaining = cap.next_wakeup(&mut child).await?;

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
