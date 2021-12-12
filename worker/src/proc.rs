use futures_util::FutureExt;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use rebuilderd_common::errors::*;
use std::cmp;
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

pub struct Capture<'a> {
    output: &'a mut Vec<u8>,
    timeout: Duration,
    size_limit: Option<usize>,
    kill_at_size_limit: bool,
    start: Instant,
    sigterm_sent: Option<Instant>,
    truncated: bool,
}

pub fn capture(output: &mut Vec<u8>, opts: Options) -> Capture<'_> {
    let start = Instant::now();
    Capture {
        output,
        timeout: opts.timeout,
        size_limit: opts.size_limit,
        kill_at_size_limit: opts.kill_at_size_limit,
        start,
        sigterm_sent: None,
        truncated: false,
    }
}

impl Capture<'_> {
    pub async fn push_bytes(&mut self, child: &mut Child, mut slice: &[u8]) -> Result<()> {
        if !self.truncated {
            if let Some(size_limit) = &self.size_limit {
                let n = cmp::min(size_limit - self.output.len(), slice.len());
                if n < 1 {
                    warn!("Exceeding output limit: output={}, slice={}, limit={}", self.output.len(), slice.len(), size_limit);
                    let msg = format!("TRUNCATED DUE TO SIZE LIMIT: {} bytes", size_limit);
                    self.truncate(child, &msg, self.kill_at_size_limit).await?;
                    return Ok(());
                } else {
                    // truncate to stay within the limit
                    slice = &slice[..n];
                }
            }

            self.output.extend(slice);
        }

        Ok(())
    }

    fn kill(pid: u32, signal: Signal) -> Result<()> {
        // convert 1234 to -1234 to kill grand-children too
        let pid = -(pid as i32);
        info!("Sending {} to child(pid={})", signal, pid);
        signal::kill(Pid::from_raw(pid), signal)?;
        Ok(())
    }

    async fn truncate(&mut self, child: &mut Child, reason: &str, kill: bool) -> Result<()> {
        if kill {
            if let Some(pid) = child.id() {
                Self::kill(pid, Signal::SIGTERM)?;
            }
            self.sigterm_sent = Some(Instant::now());
        }

        self.output.extend(format!("\n\n{}\n\n", reason).as_bytes());
        self.truncated = true;
        Ok(())
    }

    pub async fn next_wakeup(&mut self, child: &mut Child, stdout_open: &mut bool, stderr_open: &mut bool) -> Result<Duration> {
        // check if we need to SIGKILL due to SIGTERM timeout
        if let Some(sigterm_sent) = self.sigterm_sent {
            if sigterm_sent.elapsed() > Duration::from_secs(SIGKILL_DELAY) {
                if let Some(pid) = child.id() {
                    warn!("child(pid={}) didn't terminate {}s after SIGTERM, sending SIGKILL", pid, SIGKILL_DELAY);
                    // child.id is going to return None after this
                    Self::kill(pid, Signal::SIGKILL)?;
                    *stdout_open = false;
                    *stderr_open = false;
                }
            }
        }

        // check if the process timed out and we need to SIGTERM
        if let Some(remaining) = self.timeout.checked_sub(self.start.elapsed()) {
            return Ok(remaining);
        } else if self.sigterm_sent.is_none() {
            // the process has timed out, sending SIGTERM
            warn!("child timed out, killing...");
            let msg = format!("TRUNCATED DUE TO TIMEOUT: {} seconds", self.timeout.as_secs());
            self.truncate(child, &msg, true).await?;
        }

        // if we don't need any timeouts anymore we just return any value
        Ok(Duration::from_secs(SIGKILL_DELAY))
    }
}

pub async fn run<I, S>(bin: &Path, args: I, opts: Options, log: &mut Vec<u8>) -> Result<bool>
    where I: IntoIterator<Item = S> + fmt::Debug,
    S: AsRef<OsStr>,
{
    info!("Running {:?} {:?}", bin, args);
    let mut cmd = Command::new(bin);
    cmd
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(&opts.envs);

    unsafe {
        cmd.pre_exec(|| {
            // create a new process group
            let pid = nix::unistd::getpid();
            if let Err(err) = nix::unistd::setpgid(pid, Pid::from_raw(0)) {
                warn!("Failed to create new process group: {:#?}", err);
            }
            Ok(())
        });
    }

    let mut child = cmd.spawn()?;

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let mut buf_stdout = [0u8; 4096];
    let mut buf_stderr = [0u8; 4096];

    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();
    let passthrough = opts.passthrough;

    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut cap = capture(log, opts);
    let success = loop {
        let remaining = cap.next_wakeup(&mut child, &mut stdout_open, &mut stderr_open).await?;

        if stdout_open || stderr_open {
            select! {
                n = child_stdout.read(&mut buf_stdout).fuse() => {
                    let n = n?;
                    trace!("read stdout: {}", n);
                    if n == 0 {
                        stdout_open = false;
                    } else {
                        cap.push_bytes(&mut child, &buf_stdout[..n]).await?;
                        if passthrough {
                            stdout.write_all(&buf_stdout[..n]).await?;
                        }
                    }
                },
                n = child_stderr.read(&mut buf_stderr).fuse() => {
                    let n = n?;
                    trace!("read stderr: {}", n);
                    if n == 0 {
                        stderr_open = false;
                    } else {
                        cap.push_bytes(&mut child, &buf_stderr[..n]).await?;
                        if passthrough {
                            stderr.write_all(&buf_stderr[..n]).await?;
                        }
                    }
                },
                _ = time::sleep(remaining).fuse() => continue,
            }
        } else {
            select! {
                status = child.wait().fuse() => {
                    let status = status?;
                    info!("{:?} exited with exit={}, captured {} bytes", bin, status, log.len());
                    break status.success();
                }
                _ = time::sleep(remaining).fuse() => continue,
            }
        }
    };

    Ok(success)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn script(script: &str, opts: Options) -> Result<(bool, String, Duration)> {
        let start = Instant::now();
        let path = Path::new("sh");
        let mut output = Vec::new();
        let success = run(path, &["-c", script], opts, &mut output).await?;
        let duration = start.elapsed();
        let output = String::from_utf8_lossy(&output).into_owned();
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
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT: 50 bytes\n\n");
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
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT: 50 bytes\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(5));
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
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO TIMEOUT: 1 seconds\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(3));
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
            "AAAAAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAAA\n\n\nTRUNCATED DUE TO SIZE LIMIT: 50 bytes\n\n\n\nTRUNCATED DUE TO TIMEOUT: 1 seconds\n\n");
        assert!(duration > Duration::from_secs(1));
        assert!(duration < Duration::from_secs(2));
    }
}
