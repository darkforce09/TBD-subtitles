//! Series glossaries: the names and terms the language model spells right and the novelty check
//! accepts. The One Piece glossary is built in and used by default.

/// The One Piece glossary (Dressrosa arc names, places, attacks and alias traps), as JSON.
pub const ONE_PIECE_JSON: &str = include_str!("one_piece.json");

/// The built-in One Piece glossary.
pub fn one_piece() -> Vec<String> {
    parse(ONE_PIECE_JSON).unwrap_or_default()
}

/// Read a glossary: a JSON array of strings.
pub fn parse(json: &str) -> Result<Vec<String>, String> {
    let terms: Vec<String> = serde_json::from_str(json)
        .map_err(|e| format!("a glossary is a JSON array of strings: {e}"))?;
    Ok(terms
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect())
}

/// The glossary as the checks and prompts take it.
pub fn as_strs(terms: &[String]) -> Vec<&str> {
    terms.iter().map(String::as_str).collect()
}

#[cfg(test)]
#[path = "tests/glossary.rs"]
mod tests;
