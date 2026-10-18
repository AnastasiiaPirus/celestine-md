use crate::CoreError;
use std::path::{Component, Path, PathBuf};

/// Resolve a vault-relative path while ensuring existing filesystem entries stay
/// within the canonical vault root. Missing suffixes are allowed for creation.
pub fn resolve_vault_path(root: &Path, rel: &Path) -> Result<PathBuf, CoreError> {
    let canonical_root = root.canonicalize()?;
    let raw = rel.to_string_lossy().replace('\\', "/");
    if raw.as_bytes().get(1) == Some(&b':') && raw.as_bytes()[0].is_ascii_alphabetic() {
        return Err(invalid_path(rel, "absolute paths are not allowed"));
    }
    let path = Path::new(&raw);
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => components.push(part.to_os_string()),
            Component::ParentDir => {
                if components.pop().is_none() {
                    return Err(invalid_path(rel, "path escapes the vault"));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_path(rel, "absolute paths are not allowed"));
            }
        }
    }
    if components.is_empty() {
        return Err(invalid_path(rel, "vault root is not a file path"));
    }

    let mut normalized = canonical_root.clone();
    for component in &components {
        normalized.push(component);
    }

    // Inspect each existing prefix. symlink_metadata sees dangling links, whose
    // canonicalization fails instead of treating the final path as missing.
    let mut prefix = canonical_root.clone();
    for component in &components {
        prefix.push(component);
        match std::fs::symlink_metadata(&prefix) {
            Ok(_) => {
                let resolved = prefix.canonicalize().map_err(|err| {
                    CoreError::InvalidVaultPath(format!("{}: {err}", rel.display()))
                })?;
                if !resolved.starts_with(&canonical_root) {
                    return Err(invalid_path(rel, "resolved path is outside the vault"));
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
            Err(err) => return Err(err.into()),
        }
    }

    Ok(normalized)
}

fn invalid_path(path: &Path, reason: &str) -> CoreError {
    CoreError::InvalidVaultPath(format!("{}: {reason}", path.display()))
}
