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
    candidates.push(PathBuf::from(cleaned));
    if let Some(parent) = Path::new(note_rel).parent() {
        let parent_s = parent.to_string_lossy();
        if !parent_s.is_empty() && parent_s != "." {
            candidates.push(parent.join(cleaned));
        }
    }

    for rel in candidates {
        if let Ok(path) = celestine_core::resolve_vault_path(vault_root, &rel) {
            if path.is_file() {
                return Some(path);
            }
        }
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
            if let Ok(path) = celestine_core::resolve_vault_path(vault_root, Path::new(rel)) {
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub fn abs_to_rel(vault_root: &Path, abs: &Path) -> Option<String> {
    let canonical_root = vault_root.canonicalize().ok()?;
    let rel = abs.strip_prefix(&canonical_root).ok()?;
    let resolved = celestine_core::resolve_vault_path(&canonical_root, rel).ok()?;
    if !abs.canonicalize().ok()?.starts_with(&canonical_root) {
        return None;
    }
    resolved
        .strip_prefix(&canonical_root)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn image_resolution_rejects_absolute_and_escaping_destinations() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.png"), b"outside").unwrap();

        let sibling_name = outside.path().file_name().unwrap().to_string_lossy();
        let dest = format!("../{sibling_name}/outside.png");
        assert!(
            resolve_image_abs(root.path(), "notes/page.md", &dest, &[]).is_none(),
            "accepted outside destination {dest:?}"
        );
        let canonical_root = root.path().canonicalize().unwrap();
        assert!(abs_to_rel(&canonical_root, &canonical_root.join(&dest)).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn image_resolution_rejects_external_symlinks_but_accepts_internal_ones() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.png"), b"outside").unwrap();
        fs::create_dir(root.path().join("images")).unwrap();
        fs::write(root.path().join("images/inside.png"), b"inside").unwrap();
        symlink(outside.path().join("secret.png"), root.path().join("external.png")).unwrap();
        symlink(outside.path().join("missing.png"), root.path().join("dangling.png")).unwrap();
        symlink(root.path().join("images/inside.png"), root.path().join("internal.png")).unwrap();

        assert!(resolve_image_abs(root.path(), "", "external.png", &[]).is_none());
        assert!(resolve_image_abs(root.path(), "", "dangling.png", &[]).is_none());
        let resolved = resolve_image_abs(root.path(), "", "internal.png", &[]).unwrap();
        assert_eq!(fs::canonicalize(resolved).unwrap(), fs::canonicalize(root.path().join("images/inside.png")).unwrap());
    }

    #[test]
    fn image_resolution_keeps_in_vault_navigation_and_basename_search() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("images")).unwrap();
        fs::create_dir_all(root.path().join("notes/sub")).unwrap();
        fs::write(root.path().join("images/shared.png"), b"root-relative").unwrap();
        fs::write(root.path().join("notes/sub/near.png"), b"note-relative").unwrap();
        fs::write(root.path().join("notes/local.png"), b"parent navigation").unwrap();
        let known = vec!["images/shared.png".to_string()];

        for (note, dest, expected) in [
            ("notes/sub/page.md", "images/shared.png", "images/shared.png"),
            ("notes/sub/page.md", "near.png", "notes/sub/near.png"),
            ("notes/sub/page.md", "../local.png", "notes/local.png"),
            ("notes/page.md", "shared.png", "images/shared.png"),
        ] {
            let resolved = resolve_image_abs(root.path(), note, dest, &known).unwrap();
            assert_eq!(
                fs::canonicalize(resolved).unwrap(),
                fs::canonicalize(root.path().join(expected)).unwrap(),
                "wrong target for {dest:?} from {note:?}"
            );
        }
    }
}
