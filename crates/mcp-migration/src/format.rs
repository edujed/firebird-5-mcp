//! File format, compression, and codepage enums.
//!
//! This is the single source of truth for "which formats, codecs, and
//! character encodings are supported". The actual I/O (compression-aware
//! reading/writing) lives in [`crate::io`].

/// Supported data file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Parquet,
    Ipc,
    Csv,
    Dbase,
    Jsonl,
}

impl Format {
    /// Parses a format string (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "parquet" => Ok(Self::Parquet),
            "ipc" => Ok(Self::Ipc),
            "csv" => Ok(Self::Csv),
            "dbf" | "dbase" => Ok(Self::Dbase),
            "jsonl" | "ndjson" | "ldjson" => Ok(Self::Jsonl),
            other => Err(format!(
                "unsupported format '{other}' — only 'parquet', 'ipc', 'csv', 'dbf' and 'jsonl' are supported"
            )),
        }
    }

    /// Returns the canonical lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parquet => "parquet",
            Self::Ipc => "ipc",
            Self::Csv => "csv",
            Self::Dbase => "dbf",
            Self::Jsonl => "jsonl",
        }
    }

    /// Default compression codec for this format.
    pub fn default_compression(self) -> Compression {
        match self {
            Self::Parquet => Compression::Snappy,
            Self::Ipc => Compression::Zstd,
            Self::Csv => Compression::None,
            Self::Dbase => Compression::None,
            Self::Jsonl => Compression::None,
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
            Self::Csv | Self::Dbase | Self::Jsonl => &[
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

/// Detects the compression codec from a file path's extension.
///
/// Works for both CSV and DBF files:
/// - `.zst` → ZSTD
/// - `.gz` → GZIP
/// - `.lz4` → LZ4 frame
/// - anything else → None (plain)
pub fn file_compression_from_path(path: &str) -> Compression {
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

/// Character encodings (codepages) for text-based formats.
///
/// DBF is single-byte by nature — only CP437 and CP850 are valid.
/// CSV can use any of the five (multi-byte encodings are fine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodePage {
    Cp437,
    Cp850,
    Iso88591,
    Win1252,
    Utf8,
}

impl CodePage {
    /// Parses a codepage string (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "cp437" | "437" => Ok(Self::Cp437),
            "cp850" | "850" => Ok(Self::Cp850),
            "iso-8859-1" | "iso88591" | "latin1" | "latin-1" => Ok(Self::Iso88591),
            "windows-1252" | "win1252" | "cp1252" | "1252" => Ok(Self::Win1252),
            "utf-8" | "utf8" => Ok(Self::Utf8),
            other => Err(format!(
                "unsupported codepage '{other}' — valid: cp437, cp850, iso-8859-1, windows-1252, utf-8"
            )),
        }
    }

    /// Returns the canonical name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cp437 => "cp437",
            Self::Cp850 => "cp850",
            Self::Iso88591 => "iso-8859-1",
            Self::Win1252 => "windows-1252",
            Self::Utf8 => "utf-8",
        }
    }

    /// Default codepage for a format.
    pub fn default_for(format: Format) -> Self {
        match format {
            Format::Dbase => Self::Cp437,
            _ => Self::Utf8,
        }
    }

    /// Whether the format is text-based (uses codepage encoding).
    /// JSON Lines is always UTF-8 — codepage is ignored.
    pub fn is_text_based(format: Format) -> bool {
        matches!(format, Format::Csv | Format::Dbase)
    }

    /// Valid codepages for a format.
    pub fn valid_for(format: Format) -> &'static [CodePage] {
        match format {
            Format::Dbase => &[Self::Cp437, Self::Cp850],
            _ => &[
                Self::Cp437,
                Self::Cp850,
                Self::Iso88591,
                Self::Win1252,
                Self::Utf8,
            ],
        }
    }

    /// Validates that this codepage is allowed for the given format.
    pub fn validate_for(self, format: Format) -> Result<(), String> {
        if Self::valid_for(format).contains(&self) {
            Ok(())
        } else {
            Err(format!(
                "codepage '{}' is not valid for {} — valid: {}",
                self.as_str(),
                format.as_str(),
                Self::valid_for(format)
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }

    /// Encodes a UTF-8 string to the target codepage bytes.
    pub fn encode(self, s: &str) -> Vec<u8> {
        match self {
            Self::Cp437 => yore::code_pages::CP437.encode_lossy(s, b'?').to_vec(),
            Self::Cp850 => yore::code_pages::CP850.encode_lossy(s, b'?').to_vec(),
            Self::Iso88591 | Self::Win1252 => encoding_rs::WINDOWS_1252.encode(s).0.into_owned(),
            Self::Utf8 => s.as_bytes().to_vec(),
        }
    }

    /// Decodes codepage bytes to a UTF-8 string.
    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Cp437 => yore::code_pages::CP437.decode(bytes).into_owned(),
            Self::Cp850 => yore::code_pages::CP850.decode(bytes).into_owned(),
            Self::Iso88591 | Self::Win1252 => {
                encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned()
            }
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        }
    }
}
