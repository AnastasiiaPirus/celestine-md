use crate::index::{normalize_rel, NoteId, VaultIndex};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLink {
    Note(NoteId),
    Missing { target: String },
    External { url: String },
}

/// Resolve a wikilink / internal target against the index.
pub fn resolve_link(index: &VaultIndex, from_rel: &str, target: &str) -> ResolvedLink {
    let target = target.trim();
    if target.starts_with("http://") || target.starts_with("https://") {
        return ResolvedLink::External {
            url: target.to_string(),
        };
    }

    let cleaned = target
        .strip_suffix(".md")
        .unwrap_or(target)
        .replace('\\', "/");
    let key = normalize_rel(&cleaned);

    // Exact relative path from vault root
    if let Some(n) = index.get_by_rel(&cleaned) {
        return ResolvedLink::Note(n.id);
    }
    // With .md
    if let Some(n) = index.get_by_rel(&format!("{cleaned}.md")) {
        return ResolvedLink::Note(n.id);
    }

    // Relative to current note's folder
    if let Some(parent) = Path::new(from_rel).parent() {
        let joined = parent.join(&cleaned).to_string_lossy().replace('\\', "/");
        if let Some(n) = index.get_by_rel(&joined) {
            return ResolvedLink::Note(n.id);
        }
    }

    // Unique basename / stem
    let stem = Path::new(&cleaned)
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| key.clone());
    if let Some(ids) = index.stems().get(&stem) {
        if ids.len() == 1 {
            return ResolvedLink::Note(ids[0]);
        }
        // Prefer shortest path when multiple
        if let Some(best) = ids
            .iter()
            .filter_map(|id| index.get(*id))
            .min_by_key(|n| n.rel.len())
        {
            return ResolvedLink::Note(best.id);
        }
    }

    // Alias
    if let Some(ids) = index.aliases().get(&cleaned.to_lowercase()) {
        if let Some(&id) = ids.first() {
            return ResolvedLink::Note(id);
        }
    }

    ResolvedLink::Missing {
        target: cleaned,
    }
}
