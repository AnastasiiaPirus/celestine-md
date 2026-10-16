use regex::Regex;
use std::sync::OnceLock;

fn shortcode_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r":([a-zA-Z0-9_+-]+):").expect("emoji shortcode re"))
}

/// Expand Obsidian-style `:shortcode:` tokens to Unicode emoji.
/// Unknown shortcodes are left unchanged.
pub fn expand_shortcodes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    for cap in shortcode_re().captures_iter(input) {
        let m = cap.get(0).unwrap();
        out.push_str(&input[last..m.start()]);
        let name = cap.get(1).unwrap().as_str();
        if let Some(emoji) = emojis::get_by_shortcode(name) {
            out.push_str(emoji.as_str());
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&input[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_fire() {
        assert!(expand_shortcodes("hello :fire:").contains('🔥') || expand_shortcodes("hello :fire:").contains("fire"));
        // :fire: is a standard shortcode
        let s = expand_shortcodes(":fire:");
        assert_ne!(s, ":fire:");
    }
}
