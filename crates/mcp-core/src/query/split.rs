//! SQL statement splitting: `SET TERM`, strings, comments.
//!
//! Two modes:
//! - **No `SET TERM`:** strip comments, split by `;` (respecting strings and
//!   `BEGIN`/`END` blocks).
//! - **With `SET TERM`:** validate open/close pairs, isolate blocks terminated
//!   by the custom terminator. Each block becomes a separate statement.

/// Splits a SQL batch into individual statements.
///
/// Returns `Ok(Vec<String>)` on success, or `Err(String)` if the script has
/// unbalanced `SET TERM` directives.
pub fn split_statements(sql: &str) -> Result<Vec<String>, String> {
    if contains_set_term(sql) {
        split_with_set_term(sql)
    } else {
        Ok(split_without_set_term(sql))
    }
}

/// Checks if the script contains any `SET TERM` directive (outside strings/comments).
fn contains_set_term(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip strings
        if bytes[i] == b'\'' {
            i = skip_string(bytes, i, b'\'');
            continue;
        }
        if bytes[i] == b'"' {
            i = skip_string(bytes, i, b'"');
            continue;
        }
        // Skip line comments
        if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            i = skip_line_comment(bytes, i);
            continue;
        }
        // Skip block comments
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i = skip_block_comment(bytes, i);
            continue;
        }
        // Check for SET TERM
        if bytes[i].is_ascii_alphabetic() {
            let word = extract_word(sql, i);
            if word.eq_ignore_ascii_case("SET") {
                let next_i = i + word.len();
                if next_i < sql.len() {
                    let next_word = extract_word(sql, next_i);
                    if next_word.eq_ignore_ascii_case("TERM") {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

/// Skips a string starting at `i` (which is the opening quote).
/// Returns the index just past the closing quote.
fn skip_string(bytes: &[u8], i: usize, quote: u8) -> usize {
    let mut j = i + 1;
    while j < bytes.len() {
        if bytes[j] == quote {
            if bytes.get(j + 1) == Some(&quote) {
                j += 2; // escaped quote
            } else {
                return j + 1;
            }
        } else {
            j += 1;
        }
    }
    j
}

/// Skips a line comment starting at `i`.
/// Returns the index just past the newline.
fn skip_line_comment(bytes: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < bytes.len() && bytes[j] != b'\n' {
        j += 1;
    }
    if j < bytes.len() {
        j + 1 // include the newline
    } else {
        j
    }
}

/// Skips a block comment starting at `i`.
/// Returns the index just past the closing `*/`.
fn skip_block_comment(bytes: &[u8], i: usize) -> usize {
    let mut j = i + 2; // skip `/*`
    while j + 1 < bytes.len() {
        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
            return j + 2;
        }
        j += 1;
    }
    j
}

/// Extracts the word starting at `i`, skipping leading whitespace.
/// Returns the word (without whitespace).
fn extract_word(sql: &str, i: usize) -> &str {
    if i >= sql.len() {
        return "";
    }
    let rest = &sql[i..];
    let trimmed = rest.trim_start();
    let word_len = trimmed
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
        .count();
    &trimmed[..word_len]
}

/// Splits a script without `SET TERM`: strip comments, split by `;`.
/// Respects `BEGIN`/`END` blocks (semicolons inside are not separators).
fn split_without_set_term(sql: &str) -> Vec<String> {
    // Step 1: Strip comments (replace with spaces to preserve positions)
    let stripped = strip_comments(sql);

    // Step 2: Split by `;` respecting strings and BEGIN/END blocks
    let mut statements = Vec::new();
    let bytes = stripped.as_bytes();
    let mut start = 0;
    let mut i = 0;
    let mut depth = 0u32;

    while i < bytes.len() {
        if bytes[i] == b'\'' {
            i = skip_string(bytes, i, b'\'');
            continue;
        }
        if bytes[i] == b'"' {
            i = skip_string(bytes, i, b'"');
            continue;
        }

        // Track BEGIN/END depth
        if bytes[i].is_ascii_alphabetic() {
            let word = extract_word(&stripped, i);
            let word_upper = word.to_ascii_uppercase();
            if word_upper == "BEGIN" || word_upper == "CASE" {
                depth += 1;
                i += word.len();
                continue;
            }
            if word_upper == "END" {
                depth = depth.saturating_sub(1);
                i += word.len();
                continue;
            }
            // EXECUTE BLOCK starts a block that ends with END
            if word_upper == "EXECUTE" {
                let next_i = i + word.len();
                if next_i < stripped.len() {
                    let next_word = extract_word(&stripped, next_i);
                    if next_word.to_ascii_uppercase() == "BLOCK" {
                        depth += 1;
                        i += word.len();
                        continue;
                    }
                }
            }
        }

        if bytes[i] == b';' && depth == 0 {
            let stmt = stripped[start..i].trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            start = i + 1;
        }
        i += 1;
    }
    // Last statement (no trailing semicolon)
    let stmt = stripped[start..].trim();
    if !stmt.is_empty() {
        statements.push(stmt.to_string());
    }
    statements
}

/// Strips line and block comments, replacing them with spaces.
fn strip_comments(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut result = String::with_capacity(sql.len());
    let mut i = 0;
    while i < bytes.len() {
        // Skip strings (keep them as-is)
        if bytes[i] == b'\'' {
            let end = skip_string(bytes, i, b'\'');
            result.push_str(&sql[i..end]);
            i = end;
            continue;
        }
        if bytes[i] == b'"' {
            let end = skip_string(bytes, i, b'"');
            result.push_str(&sql[i..end]);
            i = end;
            continue;
        }
        // Line comment
        if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            let end = skip_line_comment(bytes, i);
            // Replace comment with spaces (preserve newlines)
            for j in i..end {
                if bytes[j] == b'\n' {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
            }
            i = end;
            continue;
        }
        // Block comment
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let end = skip_block_comment(bytes, i);
            // Replace comment with spaces (preserve newlines)
            for j in i..end {
                if bytes[j] == b'\n' {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
            }
            i = end;
            continue;
        }
        result.push(sql.as_bytes()[i] as char);
        i += 1;
    }
    result
}

/// Splits a script with `SET TERM` directives.
///
/// Validates that each `SET TERM <new> ;` has a matching `SET TERM ; <new>`.
/// Each block terminated by the custom terminator becomes a separate statement.
fn split_with_set_term(sql: &str) -> Result<Vec<String>, String> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut i = 0;
    let mut current_terminator = ";".to_string();
    let mut pending_set_term: Option<String> = None;
    let mut stmt_start = 0;

    while i < bytes.len() {
        // Skip strings
        if bytes[i] == b'\'' {
            i = skip_string(bytes, i, b'\'');
            continue;
        }
        if bytes[i] == b'"' {
            i = skip_string(bytes, i, b'"');
            continue;
        }
        // Skip line comments (update stmt_start if we're at the beginning of a statement)
        if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            let end = skip_line_comment(bytes, i);
            // If this comment is at the start of the current statement, advance stmt_start
            if sql[stmt_start..i].trim().is_empty() {
                stmt_start = end;
            }
            i = end;
            continue;
        }
        // Skip block comments (update stmt_start if we're at the beginning of a statement)
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let end = skip_block_comment(bytes, i);
            // If this comment is at the start of the current statement, advance stmt_start
            if sql[stmt_start..i].trim().is_empty() {
                stmt_start = end;
            }
            i = end;
            continue;
        }

        // Check for the current terminator FIRST (before SET TERM)
        // This ensures that terminators inside SET TERM directives are not processed
        if sql[i..].starts_with(current_terminator.as_str()) {
            let stmt = sql[stmt_start..i].trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            i += current_terminator.len();
            stmt_start = i;
            continue;
        }

        // Check for SET TERM
        if bytes[i].is_ascii_alphabetic() {
            let word = extract_word(sql, i);
            if word.eq_ignore_ascii_case("SET") {
                // Find the start of the next word (skip whitespace)
                let mut next_i = i + word.len();
                while next_i < sql.len() && sql.as_bytes()[next_i].is_ascii_whitespace() {
                    next_i += 1;
                }
                if next_i < sql.len() {
                    let next_word = extract_word(sql, next_i);
                    if next_word.eq_ignore_ascii_case("TERM") {
                        // Parse the SET TERM directive
                        let (new_term, consumed) = parse_set_term_directive(sql, i)?;

                        // Determine if this is opening or closing
                        // If we're already in a SET TERM block (pending_set_term is Some),
                        // and the new terminator is ";", this is a closing SET TERM.
                        let is_closing = pending_set_term.is_some() && new_term == ";";

                        if is_closing {
                            // This is a closing SET TERM: emit the pending statement
                            // But first, check if the current terminator appears before this SET TERM
                            // If it does, we should have split on it instead
                            let pending_term = pending_set_term.as_ref().unwrap();
                            let search_region = &sql[stmt_start..i];
                            if let Some(term_pos) = search_region.find(pending_term) {
                                // The terminator appears before the SET TERM, so we should split on it
                                let stmt = search_region[..term_pos].trim();
                                if !stmt.is_empty() {
                                    statements.push(stmt.to_string());
                                }
                                // The terminator appears before the SET TERM, so we split on it.
                                // We can't re-process from after the terminator (we're mid SET TERM
                                // block), so we emit the statement and continue from the directive.
                                pending_set_term = None;
                                current_terminator = ";".to_string();
                                stmt_start = consumed;
                                i = consumed;
                                continue;
                            }

                            let stmt = sql[stmt_start..i].trim();
                            if !stmt.is_empty() {
                                statements.push(stmt.to_string());
                            }
                            pending_set_term = None;
                            current_terminator = ";".to_string();
                            stmt_start = consumed;
                            i = consumed;
                            continue;
                        } else {
                            // This is an opening SET TERM
                            if pending_set_term.is_some() {
                                return Err(format!(
                                    "Nested SET TERM directives are not allowed. \
                                     Found 'SET TERM {}' while already inside a SET TERM block.",
                                    new_term
                                ));
                            }
                            current_terminator = new_term.clone();
                            pending_set_term = Some(new_term);
                            stmt_start = consumed;
                            i = consumed;
                            continue;
                        }
                    }
                }
            }
        }

        i += 1;
    }

    // If we still have a pending SET TERM, it was never closed
    if let Some(term) = pending_set_term {
        return Err(format!(
            "Unbalanced SET TERM: found 'SET TERM {} ;' but no matching 'SET TERM ; {}' before end of script.",
            term, term
        ));
    }

    // Handle any trailing content
    let stmt = sql[stmt_start..].trim();
    if !stmt.is_empty() {
        statements.push(stmt.to_string());
    }

    Ok(statements)
}

/// Parses a `SET TERM <new> ;` or `SET TERM ; <old>` directive at position `i`.
/// Returns the new terminator and the offset just past the directive.
fn parse_set_term_directive(sql: &str, i: usize) -> Result<(String, usize), String> {
    let rest = &sql[i..];
    let mut cursor = 0usize;

    // Skip "SET TERM"
    let set_term_len = "SET TERM".len();
    cursor += set_term_len;

    // Skip whitespace
    while cursor < rest.len() && rest.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }

    // Read the first token (could be the new terminator or `;`)
    let token_start = cursor;
    while cursor < rest.len() && !rest.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let token1 = &rest[token_start..cursor];

    // Skip whitespace
    while cursor < rest.len() && rest.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }

    // Read the second token (could be the old terminator or `;`)
    let token2_start = cursor;
    while cursor < rest.len() && !rest.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let token2 = &rest[token2_start..cursor];

    // Determine which is the new terminator
    // For "SET TERM <new> ;" (opening): token1 is the new terminator, token2 is ";"
    // For "SET TERM ; <old>" (closing): token1 is ";", token2 is the old terminator
    // We always return the terminator that will be used going forward.
    let new_term = if token1 == ";" {
        // Format: SET TERM ; <old> (closing) → new terminator is ";"
        ";".to_string()
    } else if token2 == ";" {
        // Format: SET TERM <new> ; (opening) → new terminator is token1
        token1.to_string()
    } else {
        return Err(format!(
            "Invalid SET TERM syntax: expected 'SET TERM <new> ;' or 'SET TERM ; <old>', got 'SET TERM {} {}'",
            token1, token2
        ));
    };

    Ok((new_term, i + cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_simple_statements() {
        let stmts = split_statements("SELECT 1 from RDB$Database; UPDATE t SET c = 2;").unwrap();
        assert_eq!(
            stmts,
            vec!["SELECT 1 from RDB$Database", "UPDATE t SET c = 2"]
        );
    }

    #[test]
    fn semicolon_inside_string_is_not_a_separator() {
        let stmts = split_statements(r"UPDATE t SET c = 'a;b' WHERE id = 1;").unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("a;b"));
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let stmts = split_statements(r"UPDATE t SET c = 'it''s; fine' WHERE id = 1;").unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn line_comments_are_stripped() {
        let stmts = split_statements(
            "SELECT 1 from RDB$Database; -- comment; with semicolon\nUPDATE t SET c = 2;",
        )
        .unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(!stmts[0].contains("comment"));
    }

    #[test]
    fn block_comments_are_stripped() {
        let stmts = split_statements(
            "SELECT 1 from RDB$Database; /* comment; with semicolon */ UPDATE t SET c = 2;",
        )
        .unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(!stmts[0].contains("comment"));
    }

    #[test]
    fn set_term_switches_terminator() {
        let script = "SET TERM ^ ;\nCREATE PROCEDURE SP_TEST AS BEGIN SELECT 1 from RDB$Database; END ^\nSET TERM ; ^";
        let stmts = split_statements(script).unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].starts_with("CREATE PROCEDURE"));
        assert!(stmts[0].contains("BEGIN"));
        assert!(stmts[0].contains("END"));
        // SET TERM directives must not appear in the output
        assert!(!stmts[0].contains("SET TERM"));
    }

    #[test]
    fn execute_block_survives_without_set_term() {
        let stmts =
            split_statements("EXECUTE BLOCK RETURNS (N INTEGER) AS BEGIN N = 1; SUSPEND; END;")
                .unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SUSPEND"));
    }

    #[test]
    fn nested_blocks_stay_whole() {
        let script = "CREATE PROCEDURE SP AS BEGIN IF (x = 1) THEN BEGIN y = 2; END END;";
        let stmts = split_statements(script).unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn case_counts_as_block_opener() {
        let script = "CREATE PROCEDURE SP AS BEGIN CASE WHEN x = 1 THEN y = 2; END; END;";
        let stmts = split_statements(script).unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(split_statements("").unwrap().is_empty());
        assert!(split_statements("   ").unwrap().is_empty());
        assert!(split_statements("-- only a comment").unwrap().is_empty());
    }

    #[test]
    fn trailing_semicolon_omitted() {
        let stmts = split_statements("SELECT 1 from RDB$Database;").unwrap();
        assert_eq!(stmts, vec!["SELECT 1 from RDB$Database"]);
    }

    #[test]
    fn multi_character_terminator() {
        let script = "SET TERM $$ ;\nCREATE PROCEDURE SP AS BEGIN SELECT 1 from RDB$Database; END $$\nSET TERM ; $$";
        let stmts = split_statements(script).unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].starts_with("CREATE PROCEDURE"));
    }

    #[test]
    fn unbalanced_set_term_returns_error() {
        let script = "SET TERM ^ ;\nCREATE PROCEDURE SP AS BEGIN SELECT 1 from RDB$Database; END ^";
        let result = split_statements(script);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unbalanced SET TERM"));
    }

    #[test]
    fn create_domain_with_comments() {
        let script = "-- comment\nCREATE DOMAIN D_BOOLEAN AS BOOLEAN DEFAULT TRUE NOT NULL;\nCREATE DOMAIN D_INTEGER AS INTEGER;";
        let stmts = split_statements(script).unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].starts_with("CREATE DOMAIN D_BOOLEAN"));
        assert!(stmts[1].starts_with("CREATE DOMAIN D_INTEGER"));
    }
}
