use crate::parse::ThemeTokens;
use std::path::PathBuf;

fn parse_toml_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_matches('"');
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ])
}

fn current_omarchy_theme_dir() -> Option<PathBuf> {
    // Prefer user state / env
    if let Ok(p) = std::env::var("OMARCHY_THEME_DIR") {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Some(pb);
        }
    }
    let home = dirs_next_home()?;
    // Common Omarchy state pointers
    let candidates = [
        home.join(".config/omarchy/current/theme"),
        home.join(".local/state/omarchy/current-theme"),
    ];
    for c in candidates {
        if c.is_symlink() || c.is_dir() {
            if let Ok(canon) = std::fs::canonicalize(&c) {
                return Some(canon);
            }
            if c.is_dir() {
                return Some(c);
            }
        }
        // Sometimes a file containing the slug
        if c.is_file() {
            if let Ok(slug) = std::fs::read_to_string(&c) {
                let slug = slug.trim();
                let stock = PathBuf::from("/usr/share/omarchy/themes").join(slug);
                if stock.is_dir() {
                    return Some(stock);
                }
                let user = home.join(".config/omarchy/themes").join(slug);
                if user.is_dir() {
                    return Some(user);
                }
            }
        }
    }
    // Fallback: tokyo-night stock
    let fallback = PathBuf::from("/usr/share/omarchy/themes/tokyo-night");
    if fallback.is_dir() {
        return Some(fallback);
    }
    None
}

fn dirs_next_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Load Omarchy `colors.toml` into theme tokens.
pub fn load_omarchy_terminal() -> Option<ThemeTokens> {
    let dir = current_omarchy_theme_dir()?;
    let colors = dir.join("colors.toml");
    let text = std::fs::read_to_string(colors).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let table = value.as_table()?;

    let get = |k: &str| -> Option<[u8; 3]> {
        table
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(parse_toml_hex)
    };

    Some(ThemeTokens::from_omarchy_colors(
        get("background").unwrap_or([0x1a, 0x1b, 0x26]),
        get("lighter_background").unwrap_or([0x24, 0x28, 0x3b]),
        get("foreground").unwrap_or([0xa9, 0xb1, 0xd6]),
        get("dark_foreground").unwrap_or([0x56, 0x5f, 0x89]),
        get("accent").unwrap_or([0x7a, 0xa2, 0xf7]),
        get("selection").unwrap_or([0x29, 0x2e, 0x42]),
        get("muted").unwrap_or([0x41, 0x48, 0x68]),
    ))
}
