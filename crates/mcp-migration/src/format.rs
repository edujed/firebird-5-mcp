//! File format and compression handling.
//!
//! Centralises format validation, compression codec selection, and CSV
//! I/O (with transparent compression via file extension). This is the
//! single source of truth for "which codec does this file use?" —
//! previously duplicated across `export.rs`, `csv.rs`, and `describe.rs`.

use std::fs::File;
use std::io::{BufReader, Read, Write};

/// Supported data file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Parquet,
    Ipc,
    Csv,
}

impl Format {
    /// Parses a format string (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "parquet" => Ok(Self::Parquet),
            "ipc" => Ok(Self::Ipc),
            "csv" => Ok(Self::Csv),
            other => Err(format!(
                "unsupported format '{other}' — only 'parquet', 'ipc' and 'csv' are supported"
            )),
        }
    }

    /// Returns the canonical lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parquet => "parquet",
            Self::Ipc => "ipc",
            Self::Csv => "csv",
        }
    }

    /// Default compression codec for this format.
    pub fn default_compression(self) -> Compression {
        match self {
            Self::Parquet => Compression::Snappy,
            Self::Ipc => Compression::Zstd,
            Self::Csv => Compression::None,
        }
    }

    /// Valid compression codecs for this format.
    pub fn valid_compressions(self) -> &'static [Compression] {
        match self {
            Self::Parquet => &[
                Compression::Snappy,
                Compression::Zstd,
                Compression::Lz4,
                Compression::None,
            ],
            Self::Ipc => &[Compression::Zstd, Compression::Lz4, Compression::None],
            Self::Csv => &[
                Compression::Zstd,
                Compression::Gzip,
                Compression::Lz4,
                Compression::None,
            ],
        }
    }
}

/// Compression codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Snappy,
    Zstd,
    Lz4,
    Gzip,
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
}

/// Detects the CSV compression codec from a file path's extension.
///
/// - `.csv.zst` / `.zst` → ZSTD
/// - `.csv.gz` / `.gz` → GZIP
/// - `.csv.lz4` / `.lz4` → LZ4 frame
/// - anything else → None (plain)
pub fn csv_compression_from_path(path: &str) -> Compression {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv.zst") || lower.ends_with(".zst") {
        Compression::Zstd
    } else if lower.ends_with(".csv.gz") || lower.ends_with(".gz") {
        Compression::Gzip
    } else if lower.ends_with(".csv.lz4") || lower.ends_with(".lz4") {
        Compression::Lz4
    } else {
        Compression::None
    }
}

// --- CSV reader (compression-aware) ---

/// A `Read` source that transparently decompresses based on the file
/// extension. Wraps one of: plain `File`, ZSTD decoder, GZIP decoder,
/// or LZ4 frame decoder.
pub struct CsvReader<'a> {
    inner: CsvDecoder<'a>,
}

pub enum CsvDecoder<'a> {
    Plain(File),
    Zstd(zstd::stream::read::Decoder<'a, BufReader<File>>),
    Gzip(flate2::read::GzDecoder<BufReader<File>>),
    Lz4(lz4_flex::frame::FrameDecoder<BufReader<File>>),
}

impl<'a> Read for CsvDecoder<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(f) => f.read(buf),
            Self::Zstd(d) => d.read(buf),
            Self::Gzip(d) => d.read(buf),
            Self::Lz4(d) => d.read(buf),
        }
    }
}

impl<'a> CsvReader<'a> {
    /// Opens a CSV file, wrapping it in the appropriate decoder based on
    /// the file extension.
    pub fn open(path: &str) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let inner = match csv_compression_from_path(path) {
            Compression::Zstd => CsvDecoder::Zstd(
                zstd::stream::read::Decoder::new(file)
                    .map_err(|e| format!("cannot open ZSTD decoder: {e}"))?,
            ),
            Compression::Gzip => {
                CsvDecoder::Gzip(flate2::read::GzDecoder::new(BufReader::new(file)))
            }
            Compression::Lz4 => {
                CsvDecoder::Lz4(lz4_flex::frame::FrameDecoder::new(BufReader::new(file)))
            }
            Compression::Snappy | Compression::None => CsvDecoder::Plain(file),
        };
        Ok(Self { inner })
    }

    /// Returns a `csv::Reader` over this source (flexible, with headers).
    pub fn into_csv_reader(self) -> csv::Reader<CsvDecoder<'a>> {
        csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(self.inner)
    }
}

// --- CSV writer (compression-aware) ---

/// A `Write` sink that transparently compresses based on the file
/// extension. Wraps one of: plain `File`, ZSTD encoder, GZIP encoder,
/// or LZ4 frame encoder.
pub struct CsvWriter<'a> {
    inner: CsvEncoder<'a>,
}

pub enum CsvEncoder<'a> {
    Plain(File),
    Zstd(zstd::stream::write::Encoder<'a, File>),
    Gzip(flate2::write::GzEncoder<File>),
    Lz4(lz4_flex::frame::FrameEncoder<File>),
}

impl<'a> Write for CsvEncoder<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(f) => f.write(buf),
            Self::Zstd(e) => e.write(buf),
            Self::Gzip(e) => e.write(buf),
            Self::Lz4(e) => e.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(f) => f.flush(),
            Self::Zstd(e) => e.flush(),
            Self::Gzip(e) => e.flush(),
            Self::Lz4(e) => e.flush(),
        }
    }
}

impl<'a> CsvWriter<'a> {
    /// Creates a CSV file, wrapping it in the appropriate encoder based on
    /// the file extension.
    pub fn create(path: &str) -> Result<Self, String> {
        let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
        let inner = match csv_compression_from_path(path) {
            Compression::Zstd => CsvEncoder::Zstd(
                zstd::stream::write::Encoder::new(file, 3)
                    .map_err(|e| format!("cannot open ZSTD encoder: {e}"))?,
            ),
            Compression::Gzip => CsvEncoder::Gzip(flate2::write::GzEncoder::new(
                file,
                flate2::Compression::default(),
            )),
            Compression::Lz4 => CsvEncoder::Lz4(lz4_flex::frame::FrameEncoder::new(file)),
            Compression::Snappy | Compression::None => CsvEncoder::Plain(file),
        };
        Ok(Self { inner })
    }

    /// Returns a `csv::Writer` over this sink.
    pub fn into_csv_writer(self) -> csv::Writer<CsvEncoder<'a>> {
        csv::Writer::from_writer(self.inner)
    }

    /// Flushes and finalises the underlying encoder (writes the
    /// compression trailer). Must be called after the `csv::Writer` is
    /// dropped or consumed.
    pub fn finish(inner: CsvEncoder<'a>) -> Result<(), String> {
        match inner {
            CsvEncoder::Plain(mut f) => f.flush().map_err(|e| format!("cannot flush CSV: {e}")),
            CsvEncoder::Zstd(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish ZSTD: {e}")),
            CsvEncoder::Gzip(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish GZIP: {e}")),
            CsvEncoder::Lz4(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish LZ4: {e}")),
        }
    }
}
