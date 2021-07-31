use rebuilderd_common::errors::*;
use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub fn write(buf: &[u8]) -> Result<()> {
    if atty::is(atty::Stream::Stdout) || env::var_os("NOPAGER").is_some() {
        let mut cmd = Command::new("less")
            .args(&["-R"])
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to spawn pager")?;

        if let Some(mut stdin) = cmd.stdin.take() {
            stdin.write_all(buf).ok();
        }
        cmd.wait()?;
    } else {
        io::stdout().write_all(buf).ok();
    }
    Ok(())
}
