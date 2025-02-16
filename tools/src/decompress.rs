use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use rebuilderd_common::errors::*;
use std::io::Read;
use xz2::read::XzDecoder;

#[derive(Debug, PartialEq, Eq)]
pub enum CompressedWith {
    // .gz
    Gzip,
    // .bz2
    Bzip2,
    // .xz
    Xz,
    // .zstd
    Zstd,
    Unknown,
}

pub fn detect_compression(bytes: &[u8]) -> CompressedWith {
    let mime = tree_magic_mini::from_u8(bytes);
    debug!("Detected mimetype for possibly compressed data: {:?}", mime);

    match mime {
        "application/gzip" => CompressedWith::Gzip,
        "application/x-bzip" => CompressedWith::Bzip2,
        "application/x-bzip2" => CompressedWith::Bzip2,
        "application/x-xz" => CompressedWith::Xz,
        "application/zstd" => CompressedWith::Zstd,
        _ => CompressedWith::Unknown,
    }
}

pub fn stream<'a>(comp: CompressedWith, bytes: &'a [u8]) -> Result<Box<dyn Read + 'a>> {
    match comp {
        CompressedWith::Gzip => Ok(Box::new(GzDecoder::new(bytes))),
        CompressedWith::Bzip2 => Ok(Box::new(BzDecoder::new(bytes))),
        CompressedWith::Xz => Ok(Box::new(XzDecoder::new(bytes))),
        CompressedWith::Zstd => Ok(Box::new(zstd::Decoder::new(bytes)?)),
        CompressedWith::Unknown => Ok(Box::new(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_encoding::BASE64;

    #[test]
    fn detect_no_compression() {
        let comp = detect_compression(b"ohai");
        assert_eq!(comp, CompressedWith::Unknown);
    }

    #[test]
    fn decompress_no_compression() {
        let bytes = b"ohai";
        let comp = detect_compression(bytes);
        let mut buf = Vec::new();
        stream(comp, bytes).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(bytes, b"ohai");
    }

    #[test]
    fn detect_gzip_compression() {
        let bytes = BASE64
            .decode(b"H4sIAAAAAAAAA8vPSMzkAgCKUC0+BQAAAA==")
            .unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Gzip);
    }

    #[test]
    fn decompress_gzip_compression() {
        let bytes = BASE64
            .decode(b"H4sIAAAAAAAAA8vPSMzkAgCKUC0+BQAAAA==")
            .unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Gzip);

        let mut buf = Vec::new();
        stream(comp, &bytes).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"ohai\n");
    }

    #[test]
    fn detect_bzip2_compression() {
        let bytes = BASE64
            .decode(b"QlpoOTFBWSZTWZ+CN7sAAAJBAAAQIGCgADDNAMGmwHF3JFOFCQn4I3uw")
            .unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Bzip2);
    }

    #[test]
    fn decompress_bzip2_compression() {
        let bytes = BASE64
            .decode(b"QlpoOTFBWSZTWZ+CN7sAAAJBAAAQIGCgADDNAMGmwHF3JFOFCQn4I3uw")
            .unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Bzip2);

        let mut buf = Vec::new();
        stream(comp, &bytes).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"ohai\n");
    }

    #[test]
    fn detect_xz_compression() {
        let bytes = BASE64.decode(b"/Td6WFoAAATm1rRGAgAhARYAAAB0L+WjAQAEb2hhaQoAAAAACyuekVbXbHMAAR0FuC2Arx+2830BAAAAAARZWg==").unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Xz);
    }

    #[test]
    fn decompress_xz_compression() {
        let bytes = BASE64.decode(b"/Td6WFoAAATm1rRGAgAhARYAAAB0L+WjAQAEb2hhaQoAAAAACyuekVbXbHMAAR0FuC2Arx+2830BAAAAAARZWg==").unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Xz);

        let mut buf = Vec::new();
        stream(comp, &bytes).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"ohai\n");
    }

    #[test]
    fn detect_zstd_compression() {
        let bytes = BASE64.decode(b"KLUv/QRYKQAAb2hhaQpnBE++").unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Zstd);
    }

    #[test]
    fn decompress_zstd_compression() {
        let bytes = BASE64.decode(b"KLUv/QRYKQAAb2hhaQpnBE++").unwrap();
        let comp = detect_compression(&bytes);
        assert_eq!(comp, CompressedWith::Zstd);

        let mut buf = Vec::new();
        stream(comp, &bytes).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"ohai\n");
    }
}
