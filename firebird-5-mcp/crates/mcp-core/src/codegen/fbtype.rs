//! Typed representation of a Firebird column type.
//!
//! Replaces the old `ResolvedType` (a bag of raw `RDB$FIELD_TYPE` integers
//! plus a dozen `is_*` predicates) with a proper enum. Each code-generation
//! target maps an [`FbType`] to its own language type via an *exhaustive*
//! `match`, so the compiler guarantees no Firebird type is ever forgotten.
//!
//! The raw codes are retained (in [`FbType::raw`]) so that
//! [`type_str`](FbType::type_str) can delegate to the single source of truth
//! (`types::format_column_type`) without duplicating its formatting rules.

use crate::schema::ColumnInfo;
use crate::types::format_column_type;

/// A typed Firebird column type, resolved from the raw `RDB$FIELD_TYPE`
/// codes of a [`ColumnInfo`].
#[derive(Debug, Clone, PartialEq)]
pub enum FbType {
    SmallInt,
    Integer,
    BigInt,
    Int128,
    /// `NUMERIC(p,s)` — sub-type 1.
    Numeric {
        precision: i64,
        scale: i64,
    },
    /// `DECIMAL(p,s)` — sub-type 2.
    Decimal {
        precision: i64,
        scale: i64,
    },
    /// `DECFLOAT(16)`.
    DecFloat16,
    /// `DECFLOAT(34)`.
    DecFloat34,
    Float,
    DoublePrecision,
    Char {
        size: i64,
    },
    Varchar {
        size: i64,
    },
    Nvarchar {
        size: i64,
    },
    /// C-string (type 40).
    Cstring,
    Date,
    Time,
    Timestamp,
    Boolean,
    BlobText,
    BlobBinary,
    Blob,
    /// A type code not covered above (kept as the raw `RDB$FIELD_TYPE`).
    Unknown(i64),
}

impl FbType {
    /// Resolves the typed value from a column's raw `RDB$FIELD_TYPE` codes.
    ///
    /// The classification mirrors the predicates that previously lived in
    /// `ResolvedType` (`is_blob`, `is_numeric`, `is_char`, …).
    pub fn from_column(col: &ColumnInfo) -> Self {
        let base = col.type_num;
        let sub = col.sub_type;
        let size = col.size;
        let precision = col.precision;
        let scale = col.scale;

        // BLOB (45/261): sub_type 1 = TEXT, 2 = BINARY (per types.rs).
        if base == 45 || base == 261 {
            return match sub {
                1 => Self::BlobText,
                2 => Self::BlobBinary,
                _ => Self::Blob,
            };
        }

        // NUMERIC/DECIMAL: integer base types with sub_type 1/2.
        if (base == 7 || base == 8 || base == 16 || base == 26) && (sub == 1 || sub == 2) {
            return if sub == 1 {
                Self::Numeric { precision, scale }
            } else {
                Self::Decimal { precision, scale }
            };
        }

        match base {
            7 => Self::SmallInt,
            8 => Self::Integer,
            9 | 12 => Self::Date,
            10 => Self::Time,
            11 => Self::Float,
            14 => Self::Char { size },
            16 => Self::BigInt,
            20 => Self::Varchar { size },
            23 | 49 => Self::Boolean,
            24 => Self::DecFloat16,
            25 => Self::DecFloat34,
            26 => Self::Int128,
            27 => Self::DoublePrecision,
            29 | 35 => Self::Timestamp,
            37 => Self::Varchar { size },
            40 => Self::Cstring,
            48 => Self::Nvarchar { size },
            other => Self::Unknown(other),
        }
    }

    /// The raw `RDB$FIELD_TYPE` code (and sub-type/size/precision/scale) for
    /// this type, so [`type_str`](Self::type_str) can reuse
    /// `format_column_type`.
    fn raw(&self) -> (i64, i64, i64, i64, i64) {
        match self {
            Self::SmallInt => (7, 0, 0, 0, 0),
            Self::Integer => (8, 0, 0, 0, 0),
            Self::BigInt => (16, 0, 0, 0, 0),
            Self::Int128 => (26, 0, 0, 0, 0),
            Self::Numeric { precision, scale } => (16, 1, 0, *precision, *scale),
            Self::Decimal { precision, scale } => (16, 2, 0, *precision, *scale),
            Self::DecFloat16 => (24, 0, 0, 0, 0),
            Self::DecFloat34 => (25, 0, 0, 0, 0),
            Self::Float => (11, 0, 0, 0, 0),
            Self::DoublePrecision => (27, 0, 0, 0, 0),
            Self::Char { size } => (14, 0, *size, 0, 0),
            Self::Varchar { size } => (20, 0, *size, 0, 0),
            Self::Nvarchar { size } => (48, 0, *size, 0, 0),
            Self::Cstring => (40, 0, 0, 0, 0),
            Self::Date => (9, 0, 0, 0, 0),
            Self::Time => (10, 0, 0, 0, 0),
            Self::Timestamp => (29, 0, 0, 0, 0),
            Self::Boolean => (23, 0, 0, 0, 0),
            Self::BlobText => (261, 1, 0, 0, 0),
            Self::BlobBinary => (261, 2, 0, 0, 0),
            Self::Blob => (261, 0, 0, 0, 0),
            Self::Unknown(code) => (*code, 0, 0, 0, 0),
        }
    }

    /// Returns the formatted Firebird type string (e.g. `VARCHAR(100)`,
    /// `NUMERIC(10,2)`, `BLOB SUB_TYPE TEXT`), delegating to the single
    /// source of truth in `types::format_column_type`.
    pub fn type_str(&self) -> String {
        let (base, sub, size, precision, scale) = self.raw();
        format_column_type(base, sub, size, precision, scale)
    }

    // --- Category predicates (replace the old free-standing is_* fns) ---

    pub fn is_blob(&self) -> bool {
        matches!(self, Self::BlobText | Self::BlobBinary | Self::Blob)
    }

    pub fn is_blob_text(&self) -> bool {
        matches!(self, Self::BlobText)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric { .. })
    }

    pub fn is_decimal(&self) -> bool {
        matches!(self, Self::Decimal { .. })
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float | Self::DoublePrecision)
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::SmallInt | Self::Integer | Self::BigInt | Self::Int128
        )
    }

    pub fn is_char(&self) -> bool {
        matches!(
            self,
            Self::Char { .. } | Self::Varchar { .. } | Self::Nvarchar { .. }
        )
    }

    pub fn is_date(&self) -> bool {
        matches!(self, Self::Date)
    }

    pub fn is_time(&self) -> bool {
        matches!(self, Self::Time)
    }

    pub fn is_timestamp(&self) -> bool {
        matches!(self, Self::Timestamp)
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Boolean)
    }

    /// The character size for character types (`CHAR`/`VARCHAR`/`NVARCHAR`),
    /// or `0` for non-character types.
    pub fn char_size(&self) -> i64 {
        match self {
            Self::Char { size } | Self::Varchar { size } | Self::Nvarchar { size } => *size,
            _ => 0,
        }
    }

    /// Whether this is an `NVARCHAR` (2 bytes/char) as opposed to
    /// `CHAR`/`VARCHAR` (4 bytes/char in UTF8).
    pub fn is_nvarchar(&self) -> bool {
        matches!(self, Self::Nvarchar { .. })
    }

    /// The numeric precision/scale, if this is a `NUMERIC`/`DECIMAL`.
    pub fn precision_scale(&self) -> Option<(i64, i64)> {
        match self {
            Self::Numeric { precision, scale } | Self::Decimal { precision, scale } => {
                Some((*precision, *scale))
            }
            _ => None,
        }
    }

    /// Parses a Firebird type string (e.g. `"VARCHAR(100)"`, `"NUMERIC(10,2)"`)
    /// back into an [`FbType`].
    ///
    /// This is the inverse of [`type_str`](Self::type_str) and is used by the
    /// view-DTO generator, which resolves column types from a
    /// `SELECT FIRST 1 *` result (where only the type *string* is available).
    pub fn from_type_str(s: &str) -> Self {
        let upper = s.to_uppercase();

        // BLOB variants
        if upper.starts_with("BLOB") {
            return if upper.contains("TEXT") {
                Self::BlobText
            } else if upper.contains("BINARY") {
                Self::BlobBinary
            } else {
                Self::Blob
            };
        }

        // Types with parameters: NAME(params)
        let (name, params) = if let Some(paren) = upper.find('(') {
            let close = upper[paren..].find(')').unwrap_or(upper.len() - paren);
            let params = &upper[paren + 1..paren + close];
            (&upper[..paren], params)
        } else {
            (upper.as_str(), "")
        };

        match name {
            "SMALLINT" => Self::SmallInt,
            "INTEGER" | "INT" => Self::Integer,
            "BIGINT" => Self::BigInt,
            "INT128" => Self::Int128,
            "NUMERIC" => {
                let (p, sc) = parse_precision_scale(params);
                Self::Numeric {
                    precision: p,
                    scale: sc,
                }
            }
            "DECIMAL" => {
                let (p, sc) = parse_precision_scale(params);
                Self::Decimal {
                    precision: p,
                    scale: sc,
                }
            }
            "DECFLOAT" => {
                if params == "16" {
                    Self::DecFloat16
                } else {
                    Self::DecFloat34
                }
            }
            "FLOAT" => Self::Float,
            "DOUBLE PRECISION" | "DOUBLE" => Self::DoublePrecision,
            "CHAR" => Self::Char {
                size: parse_size(params),
            },
            "VARCHAR" => Self::Varchar {
                size: parse_size(params),
            },
            "NVARCHAR" => Self::Nvarchar {
                size: parse_size(params),
            },
            "DATE" => Self::Date,
            "TIME" => Self::Time,
            "TIMESTAMP" => Self::Timestamp,
            "BOOLEAN" => Self::Boolean,
            _ => Self::Unknown(0),
        }
    }
}

/// Parses a size parameter (e.g. `"100"` → `100`).
fn parse_size(s: &str) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

/// Parses `precision,scale` (e.g. `"10,2"` → `(10, 2)`).
fn parse_precision_scale(s: &str) -> (i64, i64) {
    let parts: Vec<&str> = s.split(',').collect();
    let precision = parts
        .first()
        .and_then(|p| p.trim().parse::<i64>().ok())
        .unwrap_or(0);
    let scale = parts
        .get(1)
        .and_then(|p| p.trim().parse::<i64>().ok())
        .unwrap_or(0);
    (precision, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ColumnInfo;

    fn col(type_num: i64, sub_type: i64, size: i64, precision: i64, scale: i64) -> ColumnInfo {
        ColumnInfo {
            name: "C".to_string(),
            type_num,
            sub_type,
            size,
            precision,
            scale,
            nullable: None,
            default: None,
            position: None,
            domain: String::new(),
        }
    }

    #[test]
    fn resolves_integer_family() {
        assert_eq!(FbType::from_column(&col(7, 0, 0, 0, 0)), FbType::SmallInt);
        assert_eq!(FbType::from_column(&col(8, 0, 0, 0, 0)), FbType::Integer);
        assert_eq!(FbType::from_column(&col(16, 0, 0, 0, 0)), FbType::BigInt);
        assert_eq!(FbType::from_column(&col(26, 0, 0, 0, 0)), FbType::Int128);
    }

    #[test]
    fn resolves_numeric_decimal() {
        assert_eq!(
            FbType::from_column(&col(16, 1, 0, 10, 2)),
            FbType::Numeric {
                precision: 10,
                scale: 2
            }
        );
        assert_eq!(
            FbType::from_column(&col(16, 2, 0, 10, 2)),
            FbType::Decimal {
                precision: 10,
                scale: 2
            }
        );
    }

    #[test]
    fn resolves_char_family() {
        assert_eq!(
            FbType::from_column(&col(14, 0, 40, 0, 0)),
            FbType::Char { size: 40 }
        );
        assert_eq!(
            FbType::from_column(&col(20, 0, 400, 0, 0)),
            FbType::Varchar { size: 400 }
        );
        assert_eq!(
            FbType::from_column(&col(48, 0, 200, 0, 0)),
            FbType::Nvarchar { size: 200 }
        );
    }

    #[test]
    fn resolves_blob_family() {
        assert_eq!(FbType::from_column(&col(261, 1, 0, 0, 0)), FbType::BlobText);
        assert_eq!(
            FbType::from_column(&col(261, 2, 0, 0, 0)),
            FbType::BlobBinary
        );
        assert_eq!(FbType::from_column(&col(45, 0, 0, 0, 0)), FbType::Blob);
    }

    #[test]
    fn resolves_temporal_and_boolean() {
        assert_eq!(FbType::from_column(&col(9, 0, 0, 0, 0)), FbType::Date);
        assert_eq!(FbType::from_column(&col(10, 0, 0, 0, 0)), FbType::Time);
        assert_eq!(FbType::from_column(&col(29, 0, 0, 0, 0)), FbType::Timestamp);
        assert_eq!(FbType::from_column(&col(23, 0, 0, 0, 0)), FbType::Boolean);
    }

    #[test]
    fn type_str_delegates_to_format_column_type() {
        assert_eq!(FbType::Varchar { size: 400 }.type_str(), "VARCHAR(100)");
        assert_eq!(
            FbType::Numeric {
                precision: 10,
                scale: 2
            }
            .type_str(),
            "NUMERIC(10,2)"
        );
        assert_eq!(FbType::BlobText.type_str(), "BLOB SUB_TYPE TEXT");
        assert_eq!(FbType::Integer.type_str(), "INTEGER");
    }

    #[test]
    fn from_type_str_parses_basic_types() {
        assert_eq!(FbType::from_type_str("INTEGER"), FbType::Integer);
        assert_eq!(FbType::from_type_str("SMALLINT"), FbType::SmallInt);
        assert_eq!(FbType::from_type_str("BIGINT"), FbType::BigInt);
        assert_eq!(FbType::from_type_str("BOOLEAN"), FbType::Boolean);
        assert_eq!(FbType::from_type_str("DATE"), FbType::Date);
        assert_eq!(FbType::from_type_str("TIME"), FbType::Time);
        assert_eq!(FbType::from_type_str("TIMESTAMP"), FbType::Timestamp);
        assert_eq!(
            FbType::from_type_str("DOUBLE PRECISION"),
            FbType::DoublePrecision
        );
        assert_eq!(FbType::from_type_str("FLOAT"), FbType::Float);
    }

    #[test]
    fn from_type_str_parses_parameterized_types() {
        assert_eq!(
            FbType::from_type_str("VARCHAR(100)"),
            FbType::Varchar { size: 100 }
        );
        assert_eq!(FbType::from_type_str("CHAR(40)"), FbType::Char { size: 40 });
        assert_eq!(
            FbType::from_type_str("NVARCHAR(200)"),
            FbType::Nvarchar { size: 200 }
        );
        assert_eq!(
            FbType::from_type_str("NUMERIC(10,2)"),
            FbType::Numeric {
                precision: 10,
                scale: 2
            }
        );
        assert_eq!(
            FbType::from_type_str("DECIMAL(15,4)"),
            FbType::Decimal {
                precision: 15,
                scale: 4
            }
        );
        assert_eq!(FbType::from_type_str("DECFLOAT(16)"), FbType::DecFloat16);
        assert_eq!(FbType::from_type_str("DECFLOAT(34)"), FbType::DecFloat34);
    }

    #[test]
    fn from_type_str_parses_blob() {
        assert_eq!(
            FbType::from_type_str("BLOB SUB_TYPE TEXT"),
            FbType::BlobText
        );
        assert_eq!(
            FbType::from_type_str("BLOB SUB_TYPE BINARY"),
            FbType::BlobBinary
        );
        assert_eq!(FbType::from_type_str("BLOB"), FbType::Blob);
    }

    #[test]
    fn from_type_str_is_case_insensitive() {
        assert_eq!(
            FbType::from_type_str("varchar(50)"),
            FbType::Varchar { size: 50 }
        );
        assert_eq!(
            FbType::from_type_str("numeric(8,3)"),
            FbType::Numeric {
                precision: 8,
                scale: 3
            }
        );
    }

    #[test]
    fn from_type_str_unknown_falls_back() {
        assert_eq!(FbType::from_type_str("SOMETHING_WEIRD"), FbType::Unknown(0));
    }
}
