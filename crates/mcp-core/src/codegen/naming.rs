//! Naming helpers shared by all code-generation targets.
//!
//! Firebird column/table names are conventionally `UPPER_SNAKE_CASE`; each
//! target language needs them converted to its own convention.

/// `AUT_USUARIO` → `AutUsuario`, `PESSOA_ID` → `PessoaId`
pub fn to_pascal_case(s: &str) -> String {
    let camel = to_camel_case(s);
    if camel.is_empty() {
        return String::new();
    }
    let mut chars = camel.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    first + chars.as_str()
}

/// `PESSOA_ID` → `pessoa_id`, `SENHA_HASH` → `senha_hash`, `ID` → `id`
pub fn to_snake_case(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// `EMAIL` → `email`, `SENHA_HASH` → `senhaHash`, `ID` → `id`, `PESSOA_ID` → `pessoaId`
pub fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // First part: all lowercase (handles ID → id, EMAIL → email)
            result.push_str(&part.to_lowercase());
        } else {
            // Subsequent parts: capitalize first letter, rest lowercase
            let mut c = part.chars();
            let first = c
                .next()
                .map(|f| f.to_uppercase().to_string())
                .unwrap_or_default();
            let rest = c.as_str().to_lowercase();
            result.push_str(&first);
            result.push_str(&rest);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case() {
        assert_eq!(to_pascal_case("AUT_USUARIO"), "AutUsuario");
        assert_eq!(to_pascal_case("PESSOA_ID"), "PessoaId");
        assert_eq!(to_pascal_case("ID"), "Id");
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn snake_case() {
        assert_eq!(to_snake_case("PESSOA_ID"), "pessoa_id");
        assert_eq!(to_snake_case("SENHA_HASH"), "senha_hash");
        assert_eq!(to_snake_case("ID"), "id");
    }

    #[test]
    fn camel_case() {
        assert_eq!(to_camel_case("EMAIL"), "email");
        assert_eq!(to_camel_case("SENHA_HASH"), "senhaHash");
        assert_eq!(to_camel_case("ID"), "id");
        assert_eq!(to_camel_case("PESSOA_ID"), "pessoaId");
    }
}
