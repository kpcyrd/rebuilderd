use futures_util::FutureExt;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Command, Child};
use tokio::select;
use tokio::time;

const SIGKILL_DELAY: u64 = 10;

pub struct Options {
    pub timeout: Duration,
    pub size_limit: Option<usize>,
    pub kill_at_size_limit: bool,
    pub passthrough: bool,
    pub envs: HashMap<String, String>,
}

pub struct Capture {
    output: Vec<u8>,
    timeout: Duration,
    size_limit: Option<usize>,
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
        size_limit: opts.size_limit,
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
            if let Some(size_limit) = &self.size_limit {
                if self.output.len() + slice.len() > *size_limit {
                    warn!("Exceeding output limit: output={}, slice={}, limit={}", self.output.len(), slice.len(), size_limit);
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

pub async fn run<I, S>(bin: &Path, args: I, opts: Options) -> Result<(bool, String)>
    where I: IntoIterator<Item = S> + fmt::Debug,
    S: AsRef<OsStr>,
{
    info!("Running {:?} {:?}", bin, args);
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(&opts.envs)
        .spawn()?;

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let mut buf_stdout = [0u8; 4096];
    let mut buf_stderr = [0u8; 4096];

    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();
    let passthrough = opts.passthrough;

    let mut cap = capture(opts);
    let (success, output) = loop {
        let remaining = cap.next_wakeup(&mut child).await?;

        select! {
            n = child_stdout.read(&mut buf_stdout).fuse() => {
                let n = n?;
                cap.push_bytes(&mut child, &buf_stdout[..n]).await?;
                if passthrough {
                    stdout.write_all(&buf_stdout[..n]).await?;
                }
            },
            n = child_stderr.read(&mut buf_stderr).fuse() => {
                let n = n?;
                cap.push_bytes(&mut child, &buf_stderr[..n]).await?;
                if passthrough {
                    stderr.write_all(&buf_stderr[..n]).await?;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn script(script: &str, opts: Options) -> Result<(bool, String, Duration)> {
        let start = Instant::now();
        let path = Path::new("sh");
        let (success, output) = run(path, &["-c", script], opts).await?;
        let duration = start.elapsed();
        Ok((success, output, duration))
    }

    #[tokio::test]
    async fn hello_world() {
        let (success, output, _) = script("/bin/echo hello world", Options {
            timeout: Duration::from_secs(600),
            size_limit: None,
            kill_at_size_limit: false,
            passthrough: false,
            envs: HashMap::new(),
        }).await.unwrap();
        assert!(success);
        assert_eq!(output, "hello world\n");
    }

    #[tokio::test]
    async fn size_limit_no_kill() {
        let (success, output, _) = script("
        for x in `seq 100`; do
            /bin/echo AAAAAAAAAAAAAAAAAAAAAAAA
        done
        ", Options {
            timeout: Duration::from_secs(600),
            size_limit: Some(50),
            kill_at_size_limit: false,
            passthrough: false,
            envs: HashMap::new(),
        }).await.unwrap();
        assert!(success);
        assert_eq!(output,
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT\n\n");
    }

    #[tokio::test]
    async fn size_limit_kill() {
        let (success, output, duration) = script("
        for x in `seq 100`; do
            /bin/echo AAAAAAAAAAAAAAAAAAAAAAAA
            sleep 0.5
        done
        ", Options {
            timeout: Duration::from_secs(600),
            size_limit: Some(50),
            kill_at_size_limit: true,
            passthrough: false,
            envs: HashMap::new(),
        }).await.unwrap();
        assert!(!success);
        assert_eq!(output,
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn timeout() {
        let (success, output, duration) = script("
        for x in `seq 100`; do
            /bin/echo AAAAAAAAAAAAAAAAAAAAAAAA
            sleep 1
        done
        ", Options {
            timeout: Duration::from_millis(1500),
            size_limit: None,
            kill_at_size_limit: false,
            passthrough: false,
            envs: HashMap::new(),
        }).await.unwrap();
        assert!(!success);
        assert_eq!(output,
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO TIMEOUT\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn size_limit_no_kill_but_timeout() {
        let (success, output, duration) = script("
        for x in `seq 100`; do
            /bin/echo AAAAAAAAAAAAAAAAAAAAAAAA
            sleep 0.1
        done
        ", Options {
            timeout: Duration::from_millis(1500),
            size_limit: Some(50),
            kill_at_size_limit: false,
            passthrough: false,
            envs: HashMap::new(),
        }).await.unwrap();
        assert!(!success);
        assert_eq!(output,
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT\n\n\n\nTRUNCATED DUE TO TIMEOUT\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(2));
    }
}
