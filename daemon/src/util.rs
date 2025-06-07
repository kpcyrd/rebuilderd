use std::io::{Read, Write};
use std::{cmp, io};
use zstd::{Decoder, Encoder};

pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

// zstd has an internal buffer of 128kb - attempting to fill it completely with each chunk should
// get us near-optimal throughput
pub const ZSTD_CHUNK_SIZE: usize = 1024 * 128;

/// Compresses a block of data using the zstd algorithm in asynchronous chunks, yielding in between each one.
///
/// Chunks are sized to fit within zstd's default internal buffer size.
/// ```rust
/// use std::io::{repeat, Read};
/// use rebuilderd::util::{zstd_compress, zstd_decompress, ZSTD_CHUNK_SIZE};
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

    let mut position = 0;
    while position < data.len() {
        tokio::task::yield_now().await;

        let slice = &data[position..cmp::min(position + ZSTD_CHUNK_SIZE, data.len())];
        encoder.write_all(slice)?;

        position += slice.len();
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
