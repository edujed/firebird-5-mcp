//! Compression-aware file I/O.
//!
//! Transparently decompresses/compresses based on file extension:
//! `.zst` → ZSTD, `.gz` → GZIP, `.lz4` → LZ4 frame, anything else → plain.
//!
//! Used by both CSV and DBF paths (they share the same codec strategy).

use std::fs::File;
use std::io::{BufReader, Read, Write};

use crate::format::{Compression, file_compression_from_path};

/// A `Read` source that transparently decompresses based on the file
/// extension. Wraps one of: plain `File`, ZSTD decoder, GZIP decoder,
/// or LZ4 frame decoder.
pub struct TextReader<'a> {
    inner: TextDecoder<'a>,
}

/// The underlying decoder (must be `pub` — appears in `into_csv_reader`'s
/// return type).
pub enum TextDecoder<'a> {
    Plain(File),
    Zstd(zstd::stream::read::Decoder<'a, BufReader<File>>),
    Gzip(flate2::read::GzDecoder<BufReader<File>>),
    Lz4(lz4_flex::frame::FrameDecoder<BufReader<File>>),
}

impl<'a> Read for TextDecoder<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(f) => f.read(buf),
            Self::Zstd(d) => d.read(buf),
            Self::Gzip(d) => d.read(buf),
            Self::Lz4(d) => d.read(buf),
        }
    }
}

impl<'a> TextReader<'a> {
    /// Opens a file, wrapping it in the appropriate decoder based on
    /// the file extension.
    pub fn open(path: &str) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let inner = match file_compression_from_path(path) {
            Compression::Zstd => TextDecoder::Zstd(
                zstd::stream::read::Decoder::new(file)
                    .map_err(|e| format!("cannot open ZSTD decoder: {e}"))?,
            ),
            Compression::Gzip => {
                TextDecoder::Gzip(flate2::read::GzDecoder::new(BufReader::new(file)))
            }
            Compression::Lz4 => {
                TextDecoder::Lz4(lz4_flex::frame::FrameDecoder::new(BufReader::new(file)))
            }
            Compression::Snappy | Compression::None => TextDecoder::Plain(file),
        };
        Ok(Self { inner })
    }

    /// Returns a `csv::Reader` over this source (flexible, with headers).
    pub fn into_csv_reader(self) -> csv::Reader<TextDecoder<'a>> {
        csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(self.inner)
    }

    /// Consumes the reader and returns all decompressed bytes.
    /// Used by the DBF path (which needs the full file in memory).
    pub fn into_bytes(mut self) -> Result<Vec<u8>, String> {
        let mut buf = Vec::new();
        self.inner
            .read_to_end(&mut buf)
            .map_err(|e| format!("cannot read file: {e}"))?;
        Ok(buf)
    }
}

/// A `Write` sink that transparently compresses based on the file
/// extension. Wraps one of: plain `File`, ZSTD encoder, GZIP encoder,
/// or LZ4 frame encoder.
pub struct TextWriter<'a> {
    inner: TextEncoder<'a>,
}

/// The underlying encoder (must be `pub` — appears in `into_csv_writer`'s
/// return type).
pub enum TextEncoder<'a> {
    Plain(File),
    Zstd(zstd::stream::write::Encoder<'a, File>),
    Gzip(flate2::write::GzEncoder<File>),
    Lz4(lz4_flex::frame::FrameEncoder<File>),
}

impl<'a> Write for TextEncoder<'a> {
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

impl<'a> TextWriter<'a> {
    /// Creates a file, wrapping it in the appropriate encoder based on
    /// the file extension.
    pub fn create(path: &str) -> Result<Self, String> {
        let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
        let inner = match file_compression_from_path(path) {
            Compression::Zstd => TextEncoder::Zstd(
                zstd::stream::write::Encoder::new(file, 3)
                    .map_err(|e| format!("cannot open ZSTD encoder: {e}"))?,
            ),
            Compression::Gzip => TextEncoder::Gzip(flate2::write::GzEncoder::new(
                file,
                flate2::Compression::default(),
            )),
            Compression::Lz4 => TextEncoder::Lz4(lz4_flex::frame::FrameEncoder::new(file)),
            Compression::Snappy | Compression::None => TextEncoder::Plain(file),
        };
        Ok(Self { inner })
    }

    /// Returns a `csv::Writer` over this sink.
    pub fn into_csv_writer(self) -> csv::Writer<TextEncoder<'a>> {
        csv::Writer::from_writer(self.inner)
    }

    /// Writes raw bytes to this sink (used by the DBF export path).
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        self.inner
            .write_all(data)
            .map_err(|e| format!("cannot write file: {e}"))
    }

    /// Consumes the writer, returning the inner encoder for `finish()`.
    pub fn into_inner(self) -> TextEncoder<'a> {
        self.inner
    }

    /// Flushes and finalises the underlying encoder (writes the
    /// compression trailer). Must be called after all data is written.
    pub fn finish(inner: TextEncoder<'a>) -> Result<(), String> {
        match inner {
            TextEncoder::Plain(mut f) => f.flush().map_err(|e| format!("cannot flush file: {e}")),
            TextEncoder::Zstd(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish ZSTD: {e}")),
            TextEncoder::Gzip(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish GZIP: {e}")),
            TextEncoder::Lz4(e) => e
                .finish()
                .map(|_| ())
                .map_err(|e| format!("cannot finish LZ4: {e}")),
        }
    }
}
