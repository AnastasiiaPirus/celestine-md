//! Image path helpers for vault media (preview embeds + standalone viewer).

use std::path::{Path, PathBuf};

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

pub fn is_image_ext(ext: &str) -> bool {
    let e = ext.trim_start_matches('.').to_ascii_lowercase();
    IMAGE_EXTS.iter().any(|x| *x == e)
}

pub fn is_image_rel(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(is_image_ext)
}

pub fn is_image_target(target: &str) -> bool {
    let t = target.split('#').next().unwrap_or(target).trim();
    let t = t.split('|').next().unwrap_or(t).trim();
    is_image_rel(t)
}

/// `file://` URI for egui_extras FileLoader.
pub fn path_to_file_uri(path: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        // Windows drive paths: C:/...
        format!("file:///{s}")
    }
}

/// Resolve a markdown / wikilink image destination to an absolute path under the vault.
pub fn resolve_image_abs(
    vault_root: &Path,
    note_rel: &str,
    dest: &str,
    known_image_rels: &[String],
) -> Option<PathBuf> {
    let dest = dest
        .trim()
        .strip_prefix("wikilink:")
        .unwrap_or(dest.trim());
    if dest.is_empty()
        || dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with("data:")
    {
        return None;
    }
    let cleaned = dest.replace('\\', "/");
    let cleaned = cleaned.trim_start_matches('/');

    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(vault_root.join(cleaned));
    if let Some(parent) = Path::new(note_rel).parent() {
        let parent_s = parent.to_string_lossy();
        if !parent_s.is_empty() && parent_s != "." {
            candidates.push(vault_root.join(parent).join(cleaned));
        }
    }

    if let Some(hit) = candidates.iter().find(|p| p.is_file()) {
        return Some(hit.clone());
    }

    // Basename search (Obsidian-style) against known vault images.
    let want = Path::new(cleaned)
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())?;
    for rel in known_image_rels {
        let name = Path::new(rel)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if name == want {
            let p = vault_root.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn abs_to_rel(vault_root: &Path, abs: &Path) -> Option<String> {
    abs.strip_prefix(vault_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

pub fn skip_hidden_rel(rel: &str) -> bool {
    rel.split('/').any(|p| p.starts_with('.'))
}

/// Show an image fitted to the available width (preview / inline).
pub fn show_fitted_image(ui: &mut egui::Ui, abs: &Path, max_width: f32) -> egui::Response {
    let uri = path_to_file_uri(abs);
    let max_w = max_width.max(32.0);
    ui.add(
        egui::Image::new(uri)
            .max_width(max_w)
            .maintain_aspect_ratio(true)
            .sense(egui::Sense::click()),
    )
}

/// Full-pane standalone image viewer.
pub fn show_standalone(ui: &mut egui::Ui, abs: &Path, muted: egui::Color32) {
    if !abs.is_file() {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new(format!("missing image\n{}", abs.display()))
                    .monospace()
                    .color(muted),
            );
        });
        return;
    }
    let uri = path_to_file_uri(abs);
    let avail = ui.available_size();
    ui.centered_and_justified(|ui| {
        ui.add(
            egui::Image::new(uri)
                .fit_to_exact_size(avail)
                .maintain_aspect_ratio(true),
        );
    });
}
