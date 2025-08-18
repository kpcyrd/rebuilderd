use crate::errors::*;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub fn secs_to_human(duration: i64) -> String {
    let secs = duration % 60;
    let mins = duration / 60;
    let hours = mins / 60;
    let mins = mins % 60;

    let mut out = Vec::new();
    if hours > 0 {
        out.push(format!("{:2}h", hours));
    }
    if mins > 0 || hours > 0 {
        out.push(format!("{:2}m", mins));
    }
    out.push(format!("{:2}s", secs));

    out.join(" ")
}

pub fn load_or_create<F: Fn() -> Result<Vec<u8>>>(path: &Path, func: F) -> Result<Vec<u8>> {
    let data = match OpenOptions::new()
        .mode(0o640)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            // file didn't exist yet, generate new key
            let data = func()?;
            file.write_all(&data[..])?;
            data
        }
        Err(_err) => {
            // assume the file already exists, try reading the content
            debug!("Loading data from file: {path:?}");
            fs::read(path)?
        }
    };

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secs_to_human_0s() {
        let x = secs_to_human(0);
        assert_eq!(x, " 0s");
    }

    #[test]
    fn test_secs_to_human_1s() {
        let x = secs_to_human(1);
        assert_eq!(x, " 1s");
    }

    #[test]
    fn test_secs_to_human_1m() {
        let x = secs_to_human(60);
        assert_eq!(x, " 1m  0s");
    }

    #[test]
    fn test_secs_to_human_1m_30s() {
        let x = secs_to_human(90);
        assert_eq!(x, " 1m 30s");
    }

    #[test]
    fn test_secs_to_human_10m_30s() {
        let x = secs_to_human(630);
        assert_eq!(x, "10m 30s");
    }

    #[test]
    fn test_secs_to_human_1h() {
        let x = secs_to_human(3600);
        assert_eq!(x, " 1h  0m  0s");
    }

    #[test]
    fn test_secs_to_human_12h_10m_30s() {
        let x = secs_to_human(3600 * 12 + 600 + 30);
        assert_eq!(x, "12h 10m 30s");
    }

    #[test]
    fn test_secs_to_human_100h() {
        let x = secs_to_human(3600 * 100);
        assert_eq!(x, "100h  0m  0s");
    }
}
