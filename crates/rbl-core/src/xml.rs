//! A scanner for the small XML documents rekordbox writes: the rule of an
//! intelligent playlist, and the collection it exports.
//!
//! Both are elements with quoted attributes and no text worth reading, so
//! this walks the tags in order and hands back each element's name and
//! attributes. Declarations and comments are skipped; text between
//! elements is ignored. An XML crate would be the only user of itself in
//! the workspace, and a document this shape does not need one.

/// One element of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    /// `<NAME a="b">`, or `<NAME a="b"/>` with `closed`.
    Open { name: String, attributes: Vec<(String, String)>, closed: bool },
    /// `</NAME>`.
    Close { name: String },
}

impl Tag {
    /// The element's name, whichever end this is.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Open { name, .. } | Self::Close { name } => name,
        }
    }
}

/// The elements in order. An attribute without a closing quote ends the
/// scan with what was read.
#[must_use]
pub fn tags(xml: &str) -> Vec<Tag> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        rest = rest.get(start + 1..).unwrap_or("");
        if rest.starts_with('?') || rest.starts_with('!') {
            // `<?xml ...?>` or `<!-- ... -->` or `<!DOCTYPE ...>`.
            let end = if let Some(after) = rest.strip_prefix("!--") {
                after.find("-->").map(|i| i + 3 + 3)
            } else {
                rest.find('>').map(|i| i + 1)
            };
            let Some(end) = end else { break };
            rest = rest.get(end..).unwrap_or("");
            continue;
        }
        if let Some(after) = rest.strip_prefix('/') {
            let Some(end) = after.find('>') else { break };
            out.push(Tag::Close { name: after.get(..end).unwrap_or("").trim().to_owned() });
            rest = after.get(end + 1..).unwrap_or("");
            continue;
        }
        let name_end = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(rest.len());
        let name = rest.get(..name_end).unwrap_or("").to_owned();
        rest = rest.get(name_end..).unwrap_or("");
        let mut attributes = Vec::new();
        let mut closed = false;
        loop {
            rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix("/>") {
                closed = true;
                rest = after;
                break;
            }
            if let Some(after) = rest.strip_prefix('>') {
                rest = after;
                break;
            }
            if rest.is_empty() {
                break;
            }
            let Some(eq) = rest.find('=') else {
                // A word without a value: skip it.
                let skip = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(rest.len());
                rest = rest.get(skip..).unwrap_or("");
                continue;
            };
            let key = rest.get(..eq).unwrap_or("").trim().to_owned();
            rest = rest.get(eq + 1..).unwrap_or("").trim_start();
            let Some(quote) = rest.chars().next().filter(|&c| c == '"' || c == '\'') else { break };
            rest = rest.get(1..).unwrap_or("");
            let Some(end) = rest.find(quote) else { return out };
            attributes.push((key, unescape(rest.get(..end).unwrap_or(""))));
            rest = rest.get(end + 1..).unwrap_or("");
        }
        out.push(Tag::Open { name, attributes, closed });
    }
    out
}

/// An attribute's value, by name, case-insensitively; empty when absent.
#[must_use]
pub fn attribute(attributes: &[(String, String)], name: &str) -> String {
    attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// The five XML entities and numeric references.
#[must_use]
pub fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(rest.get(..amp).unwrap_or(""));
        rest = rest.get(amp..).unwrap_or("");
        let Some(semi) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let entity = rest.get(1..semi).unwrap_or("");
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| {
                    number
                        .strip_prefix('x')
                        .map_or_else(|| number.parse::<u32>().ok(), |hex| u32::from_str_radix(hex, 16).ok())
                })
                .and_then(char::from_u32),
        };
        match replacement {
            Some(c) => out.push(c),
            None => out.push_str(rest.get(..=semi).unwrap_or("")),
        }
        rest = rest.get(semi + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// Text as an attribute value: the five entities, so any string round-trips.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn elements_come_back_in_order_with_their_attributes() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
            <!-- a comment with <angles> -->
            <DJ_PLAYLISTS Version="1.0.0"><COLLECTION Entries="1">
              <TRACK TrackID="7" Name="Tom&apos;s &quot;Song&quot;" Location='file://localhost/Music/a%20b.mp3'/>
            </COLLECTION></DJ_PLAYLISTS>"#;
        let seen = tags(doc);
        let names: Vec<&str> = seen.iter().map(Tag::name).collect();
        assert_eq!(names, vec!["DJ_PLAYLISTS", "COLLECTION", "TRACK", "COLLECTION", "DJ_PLAYLISTS"]);
        let Tag::Open { attributes, closed, .. } = &seen[2] else { panic!() };
        assert!(closed);
        assert_eq!(attribute(attributes, "name"), "Tom's \"Song\"");
        assert_eq!(attribute(attributes, "Location"), "file://localhost/Music/a%20b.mp3");
        assert_eq!(attribute(attributes, "Missing"), "");
    }

    #[test]
    fn escaping_round_trips() {
        let text = "a < b & c > \"d\" 'e'";
        assert_eq!(unescape(&escape(text)), text);
        assert_eq!(unescape("&#65;&#x42;&unknown;&"), "AB&unknown;&");
    }
}
