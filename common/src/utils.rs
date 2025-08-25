use crate::errors::*;
use std::fs::{self, OpenOptions};
use std::io;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use zstd::{Decoder, Encoder};

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

pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

// zstd has an internal buffer of 128kb - attempting to fill it completely with each chunk should
// get us near-optimal throughput
pub const ZSTD_CHUNK_SIZE: usize = 1024 * 128;

/// Compresses a block of data using the zstd algorithm in asynchronous chunks, yielding in between each one.
///
/// Chunks are sized to fit within zstd's default internal buffer size.
/// ```rust
/// use std::io::{repeat, Read};
/// use rebuilderd_common::utils::{zstd_compress, zstd_decompress, ZSTD_CHUNK_SIZE};
///
/// tokio_test::block_on(async {
/// let undersized_data = "a".repeat(ZSTD_CHUNK_SIZE - 1).into_bytes();
/// let evenly_sized_data = "a".repeat(ZSTD_CHUNK_SIZE).into_bytes();
/// let oversized_data = "a".repeat(ZSTD_CHUNK_SIZE + 1).into_bytes();
///
/// let compressed = zstd_compress(&undersized_data).await.unwrap();
/// let decompressed = zstd_decompress(&compressed).await.unwrap();
/// assert_eq!(decompressed, undersized_data, "undersized data did not survive round-trip");
///
/// let compressed = zstd_compress(&evenly_sized_data).await.unwrap();
/// let decompressed = zstd_decompress(&compressed).await.unwrap();
/// assert_eq!(decompressed, evenly_sized_data, "evenly sized data did not survive round-trip");
///
/// let compressed = zstd_compress(&oversized_data).await.unwrap();
/// let decompressed = zstd_decompress(&compressed).await.unwrap();
/// assert_eq!(decompressed, oversized_data, "oversized data did not survive round-trip");
/// })
/// ```
pub async fn zstd_compress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = Encoder::new(Vec::new(), 11)?;

    for slice in data.chunks(ZSTD_CHUNK_SIZE) {
        tokio::task::yield_now().await;
        encoder.write_all(slice)?;
    }

    encoder.finish()
}

/// Decompresses a block of data using the zstd algorithm in asynchronous chunks, yielding in between each one.
///
/// Chunks are sized to fit within zstd's default internal buffer size.
pub async fn zstd_decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = Decoder::new(data)?;
    let mut data = vec![];

    let mut buf = vec![0u8; ZSTD_CHUNK_SIZE];
    loop {
        tokio::task::yield_now().await;

        let read_bytes = decoder.read(&mut buf)?;
        if read_bytes == 0 {
            break;
        }

        data.extend_from_slice(&buf[0..read_bytes]);
    }

    Ok(data)
}

/// Checks if a block of data is compressed with the zstd algorithm.
pub fn is_zstd_compressed(data: &[u8]) -> bool {
    data.starts_with(&ZSTD_MAGIC)
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
