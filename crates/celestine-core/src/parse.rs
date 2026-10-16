use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: TagsField,
    /// Soft-read deprecated singular forms
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TagsField(pub Vec<String>);

impl<'de> Deserialize<'de> for TagsField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            One(String),
            Many(Vec<String>),
        }
        match Option::<Raw>::deserialize(deserializer)? {
            None => Ok(TagsField(vec![])),
            Some(Raw::One(s)) => Ok(TagsField(vec![s])),
            Some(Raw::Many(v)) => Ok(TagsField(v)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    pub raw: String,
    pub target: String,
    pub heading: Option<String>,
    pub block: Option<String>,
    pub display: Option<String>,
    pub embed: bool,
    /// Byte offset in body (after frontmatter stripped)
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineLink {
    pub text: String,
    pub dest: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callout {
    pub kind: String,
    pub title: Option<String>,
    pub start_line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedNote {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub headings: Vec<(u8, String, usize)>,
    pub wikilinks: Vec<Wikilink>,
    pub md_links: Vec<InlineLink>,
    pub tags: Vec<String>,
    pub callouts: Vec<Callout>,
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\[\[([^\[\]]+)\]\]|\[\[([^\[\]]+)\]\]").expect("wikilink re"))
}

fn md_link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").expect("md link re"))
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|[\s(])#([A-Za-z][\w/-]*)").expect("tag re"))
}

fn heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*$").expect("heading re"))
}

fn callout_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^>\s*\[!([^\]]+)\]([+-])?\s*(.*)$").expect("callout re"))
}

/// Split YAML frontmatter from body.
pub fn split_frontmatter(content: &str) -> (Frontmatter, String) {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !content.starts_with("---") {
        return (Frontmatter::default(), content.to_string());
    }
    let rest = &content[3..];
    let rest = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")).unwrap_or(rest);
    let end_markers = ["\n---\n", "\n---\r\n", "\n---"];
    let mut end = None;
    for m in end_markers {
        if let Some(i) = rest.find(m) {
            end = Some((i, m.len()));
            break;
        }
    }
    let Some((i, delim_len)) = end else {
        return (Frontmatter::default(), content.to_string());
    };
    let yaml = &rest[..i];
    let body = rest[i + delim_len..].to_string();
    let mut fm: Frontmatter = serde_yaml::from_str(yaml).unwrap_or_default();
    if fm.aliases.is_empty() {
        if let Some(a) = fm.alias.take() {
            fm.aliases.push(a);
        }
    }
    if fm.tags.0.is_empty() {
        if let Some(t) = fm.tag.take() {
            fm.tags.0.push(t);
        }
    }
    (fm, body)
}

fn parse_wikilink_inner(inner: &str) -> (String, Option<String>, Option<String>, Option<String>) {
    let (main, display) = match inner.split_once('|') {
        Some((m, d)) => (m.trim(), Some(d.trim().to_string())),
        None => (inner.trim(), None),
    };
    let (target, heading, block) = if let Some((t, after)) = main.split_once('#') {
        if let Some(block) = after.strip_prefix('^') {
            (t.trim().to_string(), None, Some(block.trim().to_string()))
        } else {
            // Nested headings: keep full heading path
            (t.trim().to_string(), Some(after.trim().to_string()), None)
        }
    } else {
        (main.to_string(), None, None)
    };
    let target = target
        .strip_suffix(".md")
        .unwrap_or(&target)
        .to_string();
    (target, heading, block, display)
}

pub fn extract_note_data(content: &str) -> ParsedNote {
    let (frontmatter, body) = split_frontmatter(content);
    let mut headings = Vec::new();
    let mut callouts = Vec::new();
    for (line_idx, line) in body.lines().enumerate() {
        if let Some(c) = heading_re().captures(line) {
            let level = c.get(1).unwrap().as_str().len() as u8;
            let text = c.get(2).unwrap().as_str().to_string();
            headings.push((level, text, line_idx));
        }
        if let Some(c) = callout_re().captures(line) {
            let kind = c.get(1).unwrap().as_str().trim().to_lowercase();
            let title = c.get(3).map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty());
            callouts.push(Callout {
                kind,
                title,
                start_line: line_idx,
            });
        }
    }

    let mut wikilinks = Vec::new();
    for cap in wikilink_re().captures_iter(&body) {
        let whole = cap.get(0).unwrap();
        let embed = whole.as_str().starts_with('!');
        let inner = cap
            .get(1)
            .or_else(|| cap.get(2))
            .unwrap()
            .as_str();
        let (target, heading, block, display) = parse_wikilink_inner(inner);
        wikilinks.push(Wikilink {
            raw: whole.as_str().to_string(),
            target,
            heading,
            block,
            display,
            embed,
            start: whole.start(),
            end: whole.end(),
        });
    }

    let mut md_links = Vec::new();
    for cap in md_link_re().captures_iter(&body) {
        let whole = cap.get(0).unwrap();
        // Skip images that look like ![alt](url) — still track destination
        let text = cap.get(1).unwrap().as_str().to_string();
        let dest = cap.get(2).unwrap().as_str().to_string();
        // Skip pure external http(s)
        if dest.starts_with("http://") || dest.starts_with("https://") {
            continue;
        }
        md_links.push(InlineLink {
            text,
            dest,
            start: whole.start(),
            end: whole.end(),
        });
    }

    let mut tags = frontmatter.tags.0.clone();
    for cap in tag_re().captures_iter(&body) {
        let t = cap.get(2).unwrap().as_str().to_string();
        if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            tags.push(t);
        }
    }

    ParsedNote {
        frontmatter,
        body,
        headings,
        wikilinks,
        md_links,
        tags,
        callouts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_and_wikilinks() {
        let src = r#"---
aliases:
  - Hello
tags: [a, b]
---
# Title

See [[Other Note|label]] and ![[Embed]] and [[Folder/Note#Heading]].
Also #inbox and [[Note#^block1]].
"#;
        let p = extract_note_data(src);
        assert_eq!(p.frontmatter.aliases, vec!["Hello"]);
        assert_eq!(p.tags, vec!["a", "b", "inbox"]);
        assert_eq!(p.headings.len(), 1);
        assert_eq!(p.wikilinks.len(), 4);
        assert!(p.wikilinks[1].embed);
        assert_eq!(p.wikilinks[2].heading.as_deref(), Some("Heading"));
        assert_eq!(p.wikilinks[3].block.as_deref(), Some("block1"));
    }
}
