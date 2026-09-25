//! HTML tags inside a Markdown run: where a tag ends, and the anchors an `<a>` tag declares.
//!
//! **Role:** reads one HTML open or closing tag starting at a `<`, with its attributes, and
//! reports the `id` and `name` values of an `<a>` tag, the explicit anchors a fragment may name.
//!
//! **Position:** called by the inline pass ([`super::markdown_inlines`]) whenever a `<` opens
//! neither an HTML comment nor an autolink.
//!
//! **Signals and state:** none; pure functions over characters.
//!
//! **Invariants:** CommonMark's tag shape: a tag name of ASCII letters, digits and hyphens starting
//! with a letter; attributes separated by whitespace, each a name with an optional unquoted,
//! single- or double-quoted value; an open tag may close with `/>`; anything else is no tag.

/// An HTML open or closing tag at `index`: the index of its `>` and the anchors an `<a>` tag's
/// `id` and `name` attributes declare.
pub(super) fn html_tag(chars: &[char], index: usize) -> Option<(usize, Vec<String>)> {
    let mut at = index + 1;
    let closing = chars.get(at) == Some(&'/');
    if closing {
        at += 1;
    }
    let name_start = at;
    if !chars.get(at)?.is_ascii_alphabetic() {
        return None;
    }
    while chars
        .get(at)
        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '-')
    {
        at += 1;
    }
    let is_anchor = at - name_start == 1 && chars[name_start].eq_ignore_ascii_case(&'a');
    let mut anchors = Vec::new();
    loop {
        let spaced = chars.get(at).is_some_and(|c| c.is_whitespace());
        while chars.get(at).is_some_and(|c| c.is_whitespace()) {
            at += 1;
        }
        match *chars.get(at)? {
            '>' => return Some((at, anchors)),
            '/' if !closing && chars.get(at + 1) == Some(&'>') => return Some((at + 1, anchors)),
            c if spaced && !closing && (c.is_ascii_alphabetic() || c == '_' || c == ':') => {
                let (name, value, next) = attribute(chars, at)?;
                let declares_anchor =
                    name.eq_ignore_ascii_case("id") || name.eq_ignore_ascii_case("name");
                if is_anchor && declares_anchor {
                    anchors.extend(value.filter(|value| !value.is_empty()));
                }
                at = next;
            }
            _ => return None,
        }
    }
}

/// One attribute at `at`: its name, its value when it has one, and the index past it.
fn attribute(chars: &[char], at: usize) -> Option<(String, Option<String>, usize)> {
    let mut index = at;
    while chars
        .get(index)
        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
    {
        index += 1;
    }
    let name: String = chars[at..index].iter().collect();
    let mut after = index;
    while chars.get(after).is_some_and(|c| c.is_whitespace()) {
        after += 1;
    }
    if chars.get(after) != Some(&'=') {
        return Some((name, None, index));
    }
    after += 1;
    while chars.get(after).is_some_and(|c| c.is_whitespace()) {
        after += 1;
    }
    let quote = *chars.get(after)?;
    if quote == '"' || quote == '\'' {
        let length = chars[after + 1..].iter().position(|c| *c == quote)?;
        let value: String = chars[after + 1..after + 1 + length].iter().collect();
        return Some((name, Some(value), after + length + 2));
    }
    let length = chars[after..]
        .iter()
        .take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | '=' | '<' | '>' | '`'))
        .count();
    if length == 0 {
        return None;
    }
    let value: String = chars[after..after + length].iter().collect();
    Some((name, Some(value), after + length))
}

#[cfg(test)]
#[path = "tests/inline_html.rs"]
mod tests;
