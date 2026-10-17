//! SQL statement splitting: `SET TERM`, `BEGIN`/`END` nesting, strings, comments.
//!
//! Ported from Plamenix (`plamenix-db/src/query.rs`).

/// Splits a SQL batch into individual statements at the active terminator.
///
/// Honours Firebird string and identifier quoting (`'foo''bar'` and
/// `"id \"\"esc\"\""`), line comments (`-- to end of line`), and block
/// comments (`/* ... */`). Comment-only or whitespace-only statements
/// are dropped.
///
/// Procedural SQL needs two extra rules:
///
/// * **`SET TERM`** switches the terminator, exactly as `isql` and
///   IBExpert scripts expect. The directive is consumed rather than
///   emitted — it is a client instruction the server has never heard of.
/// * **`BEGIN` / `END` nesting** means a semicolon inside a PSQL body is
///   just a statement separator within that body, so a block stays whole
///   even when the script never sets a terminator. `CASE` counts as an
///   opener too, since it also closes with `END`.
#[must_use]
pub fn split_statements(sql: &str) -> Vec<String> {
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut state = State::Normal;
    let mut start = 0usize;
    let mut has_content = false;
    let mut out = Vec::new();
    let mut terminator: String = ";".to_string();
    let mut depth = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match state {
            State::Normal => {
                if !has_content {
                    if let Some((new_term, consumed)) = parse_set_term(sql, i, &terminator) {
                        terminator = new_term;
                        i = consumed;
                        start = i;
                        continue;
                    }
                }
                match c {
                    b'\'' => {
                        state = State::SingleQuote;
                        has_content = true;
                    }
                    b'"' => {
                        state = State::DoubleQuote;
                        has_content = true;
                    }
                    b'-' if bytes.get(i + 1).copied() == Some(b'-') => {
                        state = State::LineComment;
                        i += 1;
                    }
                    b'/' if bytes.get(i + 1).copied() == Some(b'*') => {
                        state = State::BlockComment;
                        i += 1;
                    }
                    _ => {
                        if depth == 0 && sql[i..].starts_with(terminator.as_str()) {
                            if has_content {
                                out.push(sql[start..i].trim().to_string());
                            }
                            i += terminator.len();
                            start = i;
                            has_content = false;
                            continue;
                        }
                        if let Some(word_len) = keyword_at(sql, bytes, i) {
                            match &sql[i..i + word_len].to_ascii_uppercase()[..] {
                                "BEGIN" | "CASE" => depth += 1,
                                "END" => depth = depth.saturating_sub(1),
                                _ => {}
                            }
                            has_content = true;
                            i += word_len;
                            continue;
                        }
                        if !c.is_ascii_whitespace() {
                            has_content = true;
                        }
                    }
                }
            }
            State::SingleQuote => {
                if c == b'\'' {
                    if bytes.get(i + 1).copied() == Some(b'\'') {
                        i += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::DoubleQuote => {
                if c == b'"' {
                    if bytes.get(i + 1).copied() == Some(b'"') {
                        i += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::LineComment => {
                if c == b'\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                if c == b'*' && bytes.get(i + 1).copied() == Some(b'/') {
                    state = State::Normal;
                    i += 1;
                }
            }
        }
        i += 1;
    }
    if has_content {
        out.push(sql[start..].trim().to_string());
    }
    out
}

/// Length of the identifier-like word starting at `i`, or `None` when
/// `i` is not on a word boundary.
fn keyword_at(sql: &str, bytes: &[u8], i: usize) -> Option<usize> {
    fn is_word_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
    }
    if !bytes[i].is_ascii_alphabetic() {
        return None;
    }
    if i > 0 && is_word_byte(bytes[i - 1]) {
        return None;
    }
    let len = sql[i..].bytes().take_while(|b| is_word_byte(*b)).count();
    Some(len)
}

/// Recognises a `SET TERM <new> <current>` directive at `i`.
///
/// Returns the new terminator and the offset just past the directive,
/// or `None` when this is ordinary SQL.
fn parse_set_term(sql: &str, i: usize, current: &str) -> Option<(String, usize)> {
    let rest = &sql[i..];
    let mut cursor = 0usize;

    let expect_word = |word: &str, cursor: &mut usize| -> bool {
        let tail = &rest[*cursor..];
        let trimmed = tail.trim_start();
        let skipped = tail.len() - trimmed.len();
        if trimmed.len() < word.len() || !trimmed[..word.len()].eq_ignore_ascii_case(word) {
            return false;
        }
        let after = trimmed.as_bytes().get(word.len()).copied();
        if after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$') {
            return false;
        }
        *cursor += skipped + word.len();
        true
    };

    if !expect_word("SET", &mut cursor) || !expect_word("TERM", &mut cursor) {
        return None;
    }

    let tail = &rest[cursor..];
    let trimmed = tail.trim_start();
    cursor += tail.len() - trimmed.len();
    let token_len = trimmed
        .bytes()
        .take_while(|b| !b.is_ascii_whitespace())
        .count();
    if token_len == 0 {
        return None;
    }
    let token = &trimmed[..token_len];
    cursor += token_len;

    let new_term = match token.strip_suffix(current) {
        Some(prefix) if !prefix.is_empty() => prefix.to_string(),
        _ => {
            let tail = &rest[cursor..];
            let trimmed = tail.trim_start();
            cursor += tail.len() - trimmed.len();
            if trimmed.starts_with(current) {
                cursor += current.len();
            }
            token.to_string()
        }
    };

    Some((new_term, i + cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_simple_statements() {
        let stmts = split_statements("SELECT 1; UPDATE t SET c = 2;");
        assert_eq!(stmts, vec!["SELECT 1", "UPDATE t SET c = 2"]);
    }

    #[test]
    fn semicolon_inside_string_is_not_a_separator() {
        let stmts = split_statements(r"UPDATE t SET c = 'a;b' WHERE id = 1;");
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("a;b"));
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let stmts = split_statements(r"UPDATE t SET c = 'it''s; fine' WHERE id = 1;");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn line_comments_swallow_semicolons() {
        let stmts = split_statements("SELECT 1; -- comment; with semicolon\nUPDATE t SET c = 2;");
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn block_comments_swallow_semicolons() {
        let stmts = split_statements("SELECT 1; /* comment; with semicolon */ UPDATE t SET c = 2;");
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn set_term_switches_terminator() {
        let script =
            "SET TERM ^ ;\nCREATE PROCEDURE SP_TEST AS BEGIN SELECT 1; END ^\nSET TERM ; ^";
        let stmts = split_statements(script);
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
            split_statements("EXECUTE BLOCK RETURNS (N INTEGER) AS BEGIN N = 1; SUSPEND; END;");
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SUSPEND"));
    }

    #[test]
    fn nested_blocks_stay_whole() {
        let script = "CREATE PROCEDURE SP AS BEGIN IF (x = 1) THEN BEGIN y = 2; END END;";
        let stmts = split_statements(script);
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn case_counts_as_block_opener() {
        let script = "CREATE PROCEDURE SP AS BEGIN CASE WHEN x = 1 THEN y = 2; END; END;";
        let stmts = split_statements(script);
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(split_statements("").is_empty());
        assert!(split_statements("   ").is_empty());
        assert!(split_statements("-- only a comment").is_empty());
    }

    #[test]
    fn trailing_semicolon_omitted() {
        let stmts = split_statements("SELECT 1;");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn multi_character_terminator() {
        let script = "SET TERM $$ ;\nCREATE PROCEDURE SP AS BEGIN SELECT 1; END $$\nSET TERM ; $$";
        let stmts = split_statements(script);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].starts_with("CREATE PROCEDURE"));
    }
}
