//! CSS custom-property theme packs for CelestineMD.

mod omarchy;
mod parse;

pub use omarchy::load_omarchy_terminal;
pub use parse::{parse_css_vars, ThemeTokens};

use std::path::{Path, PathBuf};

pub const BUILTIN_THEME_NAMES: &[&str] = &[
    "obsidian-dark",
    "obsidian-light",
    "catppuccin",
    "tokyo-night",
    "gruvbox-dark",
    "gruvbox-light",
    "nord",
    "solarized-dark",
    "solarized-light",
    "dracula",
    "rose-pine",
    "everforest-dark",
    "terminal",
];

pub fn themes_dir_candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(proj) = directories::ProjectDirs::from("md", "Celestine", "celestine") {
        dirs.push(proj.data_dir().join("themes"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/celestine/themes"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("themes"));
            dirs.push(parent.join("../share/celestine/themes"));
            dirs.push(parent.join("../themes"));
            dirs.push(parent.join("../../themes"));
        }
    }
    dirs.push(PathBuf::from("themes"));
    dirs.push(PathBuf::from("../themes"));
    dirs.push(PathBuf::from("../../themes"));
    // CARGO_MANIFEST_DIR when running from source
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"));
    dirs
}

pub fn find_themes_dir() -> Option<PathBuf> {
    themes_dir_candidates().into_iter().find(|d| d.is_dir())
}

pub fn load_theme(name: &str) -> Result<ThemeTokens, String> {
    if name == "terminal" {
        if let Some(t) = load_omarchy_terminal() {
            return Ok(t);
        }
        return Ok(ThemeTokens::terminal_fallback());
    }
    let dir = find_themes_dir().ok_or_else(|| "themes directory not found".to_string())?;
    let path = dir.join(format!("{name}.css"));
    let mut t = load_theme_file(&path)?;
    t.name = name.to_string();
    Ok(t)
}

pub fn load_theme_file(path: &Path) -> Result<ThemeTokens, String> {
    let css = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut t = parse_css_vars(&css);
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        t.name = stem.to_string();
    }
    Ok(t)
}

pub fn list_available_themes() -> Vec<String> {
    let mut names: Vec<String> = BUILTIN_THEME_NAMES.iter().map(|s| (*s).to_string()).collect();
    if let Some(dir) = find_themes_dir() {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("css") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        if !names.iter().any(|n| n == stem) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}
