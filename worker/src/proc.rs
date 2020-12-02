use futures_util::FutureExt;
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use rebuilderd_common::errors::*;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::{Command, Child};
use tokio::select;
use tokio::time;

const SIGKILL_DELAY: u64 = 10;

pub struct Options {
    pub timeout: Duration,
    pub limit: Option<usize>,
    pub kill_at_size_limit: bool,
}

pub struct Capture {
    output: Vec<u8>,
    timeout: Duration,
    limit: Option<usize>,
    kill_at_size_limit: bool,
    start: Instant,
    sigterm_sent: Option<Instant>,
    truncated: bool,
}

pub fn capture(opts: Options) -> Capture {
    let start = Instant::now();
    Capture {
        output: Vec::new(),
        timeout: opts.timeout,
        limit: opts.limit,
        kill_at_size_limit: opts.kill_at_size_limit,
        start,
        sigterm_sent: None,
        truncated: false,
    }
}

impl Capture {
    #[inline]
    pub fn into_output(self) -> Vec<u8> {
        self.output
    }

    pub async fn push_bytes(&mut self, child: &mut Child, slice: &[u8]) -> Result<()> {
        if !self.truncated {
            if let Some(limit) = &self.limit {
                if self.output.len() + slice.len() > *limit {
                    warn!("Exceeding output limit: output={}, slice={}, limit={}", self.output.len(), slice.len(), limit);
                    self.truncate(child, "TRUNCATED DUE TO SIZE LIMIT", self.kill_at_size_limit).await?;
                    return Ok(());
                }
            }

            self.output.extend(slice);
        }

        Ok(())
    }

    async fn truncate(&mut self, child: &mut Child, reason: &str, kill: bool) -> Result<()> {
        if kill {
            if let Some(pid) = child.id() {
                info!("Sending SIGTERM to child(pid={})", pid);
                signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)?;
            }
            self.sigterm_sent = Some(Instant::now());
        }

        self.output.extend(format!("\n\n{}\n\n", reason).as_bytes());
        self.truncated = true;
        Ok(())
    }

    pub async fn next_wakeup(&mut self, child: &mut Child) -> Result<Duration> {
        // check if we need to SIGKILL due to SIGTERM timeout
        if let Some(sigterm_sent) = self.sigterm_sent {
            if sigterm_sent.elapsed() > Duration::from_secs(SIGKILL_DELAY) {
                if let Some(pid) = child.id() {
                    warn!("child(pid={}) didn't terminate {}s after SIGTERM, sending SIGKILL", pid, SIGKILL_DELAY);
                    // child.id is going to return None after this
                    child.kill().await?;
                }
            }
        }

        // check if the process timed out and we need to SIGTERM
        if let Some(remaining) = self.timeout.checked_sub(self.start.elapsed()) {
            return Ok(remaining);
        } else if self.sigterm_sent.is_none() {
            // the process has timed out, sending SIGTERM
            warn!("child timed out, killing...");
            self.truncate(child, "TRUNCATED DUE TO TIMEOUT", true).await?;
        }

        // if we don't need any timeouts anymore we just return any value
        Ok(Duration::from_secs(SIGKILL_DELAY))
    }
}

pub async fn run(bin: &Path, args: &[&str], opts: Options) -> Result<(bool, String)> {
    info!("Running {:?} {:?}", bin, args);
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let mut buf_stdout = [0u8; 4096];
    let mut buf_stderr = [0u8; 4096];

    let mut cap = capture(opts);
    let (success, output) = loop {
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
                info!("{:?} exited with exit={}, captured {} bytes", bin, status, output.len());
                break (status.success(), output);
            }
            _ = time::sleep(remaining).fuse() => continue,
        }
    };

    let output = String::from_utf8_lossy(&output);
    Ok((success, output.into_owned()))
}
