//! Shared compression utilities.
//!
//! Provides the `Compression` enum, extension-based detection, and
//! byte-level compress/decompress functions used by both the migration
//! crate (CSV/DBF/JSONL export/import) and the storage module
//! (generated file compression).
//!
//! Supported codecs:
//! - `.zst` → ZSTD
//! - `.gz` → GZIP
//! - `.lz4` → LZ4 frame
//! - anything else → plain (no compression)

/// Compression codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Snappy (Parquet only — handled by the parquet crate, not here).
    Snappy,
    /// Zstandard (`.zst`).
    Zstd,
    /// LZ4 frame (`.lz4`).
    Lz4,
    /// GZIP (`.gz`).
    Gzip,
    /// No compression (plain file).
    None,
}

impl Compression {
    /// Parses a compression string (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "snappy" => Ok(Self::Snappy),
            "zstd" => Ok(Self::Zstd),
            "lz4" => Ok(Self::Lz4),
            "gzip" => Ok(Self::Gzip),
            "none" => Ok(Self::None),
            other => Err(format!("unsupported compression '{other}'")),
        }
    }

    /// Returns the canonical lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snappy => "snappy",
            Self::Zstd => "zstd",
            Self::Lz4 => "lz4",
            Self::Gzip => "gzip",
            Self::None => "none",
        }
    }

    /// Returns the file extension suffix for this codec (e.g. `.zst`).
    /// `None` and `Snappy` return `None` (no file-level suffix).
    pub fn suffix(self) -> Option<&'static str> {
        match self {
            Self::Zstd => Some(".zst"),
            Self::Gzip => Some(".gz"),
            Self::Lz4 => Some(".lz4"),
            Self::Snappy | Self::None => None,
        }
    }
}

/// Detects the compression codec from a file path's extension.
///
/// - `.zst` → ZSTD
/// - `.gz` → GZIP
/// - `.lz4` → LZ4 frame
/// - anything else → None (plain)
pub fn from_path(path: &str) -> Compression {
    let lower = path.to_lowercase();
    if lower.ends_with(".zst") {
        Compression::Zstd
    } else if lower.ends_with(".gz") {
        Compression::Gzip
    } else if lower.ends_with(".lz4") {
        Compression::Lz4
    } else {
        Compression::None
    }
}

/// Compresses bytes using the specified codec.
///
/// `Compression::None` and `Compression::Snappy` return the input unchanged
/// (Snappy is handled at the Parquet level, not file level).
pub fn compress(data: &[u8], codec: Compression) -> Result<Vec<u8>, String> {
    match codec {
        Compression::Zstd => {
            use std::io::Write;
            let mut encoder = zstd::Encoder::new(Vec::new(), 3).map_err(|e| e.to_string())?;
            encoder
                .write_all(data)
                .and_then(|_| encoder.finish())
                .map_err(|e| e.to_string())
        }
        Compression::Gzip => {
            use std::io::Write;
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder
                .write_all(data)
                .and_then(|_| encoder.finish())
                .map_err(|e| e.to_string())
        }
        Compression::Lz4 => {
            use std::io::Write;
            let mut frame = lz4_flex::frame::FrameEncoder::new(Vec::new());
            frame.write_all(data).map_err(|e| e.to_string())?;
            frame.finish().map_err(|e| e.to_string())
        }
        Compression::Snappy | Compression::None => Ok(data.to_vec()),
    }
}

/// Decompresses bytes using the specified codec.
///
/// `Compression::None` and `Compression::Snappy` return the input unchanged.
pub fn decompress(data: &[u8], codec: Compression) -> Result<Vec<u8>, String> {
    match codec {
        Compression::Zstd => {
            use std::io::Read;
            let mut decoder = zstd::stream::read::Decoder::new(data).map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        Compression::Gzip => {
            use std::io::Read;
            let mut decoder = flate2::read::GzDecoder::new(data);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        Compression::Lz4 => {
            use std::io::Read;
            let mut decoder = lz4_flex::frame::FrameDecoder::new(data);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        Compression::Snappy | Compression::None => Ok(data.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path() {
        assert_eq!(from_path("file.zst"), Compression::Zstd);
        assert_eq!(from_path("file.gz"), Compression::Gzip);
        assert_eq!(from_path("file.lz4"), Compression::Lz4);
        assert_eq!(from_path("file.csv"), Compression::None);
        assert_eq!(from_path("file.sql"), Compression::None);
        assert_eq!(from_path("FILE.ZST"), Compression::Zstd);
    }

    #[test]
    fn test_parse() {
        assert_eq!(Compression::parse("zstd"), Ok(Compression::Zstd));
        assert_eq!(Compression::parse("gzip"), Ok(Compression::Gzip));
        assert_eq!(Compression::parse("lz4"), Ok(Compression::Lz4));
        assert_eq!(Compression::parse("none"), Ok(Compression::None));
        assert_eq!(Compression::parse("snappy"), Ok(Compression::Snappy));
        assert!(Compression::parse("brotli").is_err());
    }

    #[test]
    fn test_suffix() {
        assert_eq!(Compression::Zstd.suffix(), Some(".zst"));
        assert_eq!(Compression::Gzip.suffix(), Some(".gz"));
        assert_eq!(Compression::Lz4.suffix(), Some(".lz4"));
        assert_eq!(Compression::None.suffix(), None);
        assert_eq!(Compression::Snappy.suffix(), None);
    }

    #[test]
    fn test_zstd_roundtrip() {
        let data = b"hello world, this is a test of ZSTD compression";
        let compressed = compress(data, Compression::Zstd).unwrap();
        assert!(compressed.len() < data.len() + 10); // overhead is small
        let decompressed = decompress(&compressed, Compression::Zstd).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_gzip_roundtrip() {
        let data = b"hello world, this is a test of GZIP compression";
        let compressed = compress(data, Compression::Gzip).unwrap();
        let decompressed = decompress(&compressed, Compression::Gzip).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_roundtrip() {
        let data = b"hello world, this is a test of LZ4 compression";
        let compressed = compress(data, Compression::Lz4).unwrap();
        let decompressed = decompress(&compressed, Compression::Lz4).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_none_passthrough() {
        let data = b"plain data";
        assert_eq!(compress(data, Compression::None).unwrap(), data);
        assert_eq!(decompress(data, Compression::None).unwrap(), data);
    }

    #[test]
    fn test_snappy_passthrough() {
        let data = b"snappy is handled at parquet level";
        assert_eq!(compress(data, Compression::Snappy).unwrap(), data);
        assert_eq!(decompress(data, Compression::Snappy).unwrap(), data);
    }
}
