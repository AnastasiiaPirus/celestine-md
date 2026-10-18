use crate::graph::LinkGraph;
use crate::index::VaultIndex;
use crate::CoreError;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use walkdir::WalkDir;

pub struct Vault {
    pub root: PathBuf,
    pub index: VaultIndex,
    pub graph: LinkGraph,
    _watcher: Option<RecommendedWatcher>,
    rx: Option<Receiver<Result<Event, notify::Error>>>,
}

impl Vault {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref().canonicalize().unwrap_or_else(|_| root.as_ref().to_path_buf());
        if !root.is_dir() {
            return Err(CoreError::NotADirectory(root));
        }
        let mut index = VaultIndex::new();
        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if !is_md_path(path) {
                continue;
            }
            let _ = index.upsert_file(&root, path);
        }
        let graph = LinkGraph::rebuild(&index);

        let (tx, rx) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )
        .ok()
        .and_then(|mut w| {
            if w.watch(&root, RecursiveMode::Recursive).is_ok() {
                Some(w)
            } else {
                None
            }
        });

        Ok(Self {
            root,
            index,
            graph,
            _watcher: watcher,
            rx: Some(rx),
        })
    }

    pub fn rebuild_graph(&mut self) {
        self.graph = LinkGraph::rebuild(&self.index);
    }

    /// Drain filesystem events and update index.
    pub fn poll_fs(&mut self) -> bool {
        let mut paths = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(Ok(event)) = rx.try_recv() {
                if matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    paths.extend(event.paths);
                }
            }
        }
        let mut dirty = false;
        for path in paths {
            dirty |= self.apply_fs_path(&path);
        }
        if dirty {
            self.rebuild_graph();
        }
        dirty
    }

    /// Bring the index in line with the filesystem for one event path.
    ///
    /// Notes are upserted or dropped directly. For anything else — a folder
    /// that was renamed, moved or deleted, or a path that no longer exists —
    /// the watcher only tells us about the folder, so drop every entry whose
    /// file is gone and index whatever now lives under the path.
    fn apply_fs_path(&mut self, path: &Path) -> bool {
        if is_md_path(path) {
            if path.is_file() {
                let _ = self.index.upsert_file(&self.root, path);
            } else {
                self.index.remove_path(&self.root, path);
            }
            return true;
        }
        if path.is_file() {
            // Image or other asset: not indexed.
            return false;
        }
        let mut changed = self.prune_missing_notes();
        if path.is_dir() {
            changed |= self.index_tree(path);
        }
        changed
    }

    /// Drop index entries whose files no longer exist on disk.
    fn prune_missing_notes(&mut self) -> bool {
        let gone: Vec<PathBuf> = self
            .index
            .notes()
            .filter(|n| !n.path.is_file())
            .map(|n| n.path.clone())
            .collect();
        for path in &gone {
            self.index.remove_path(&self.root, path);
        }
        !gone.is_empty()
    }

    /// Index every note under `dir`.
    fn index_tree(&mut self, dir: &Path) -> bool {
        let mut changed = false;
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if is_md_path(entry.path()) && self.index.upsert_file(&self.root, entry.path()).is_ok() {
                changed = true;
            }
        }
        changed
    }

    pub fn read_note_content(&self, rel: &str) -> Result<String, CoreError> {
        let path = self.root.join(rel);
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn write_note(&mut self, rel: &str, content: &str) -> Result<(), CoreError> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        let _ = self.index.upsert_file(&self.root, &path);
        self.rebuild_graph();
        Ok(())
    }

    pub fn create_note(&mut self, rel: &str) -> Result<(), CoreError> {
        let mut rel = rel.to_string();
        if !rel.to_lowercase().ends_with(".md") {
            rel.push_str(".md");
        }
        let path = self.root.join(&rel);
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into());
        let content = format!("# {title}\n\n");
        std::fs::write(&path, content)?;
        let _ = self.index.upsert_file(&self.root, &path);
        self.rebuild_graph();
        Ok(())
    }

    pub fn list_md_rel_paths(&self) -> Vec<String> {
        let mut v: Vec<_> = self.index.notes().map(|n| n.rel.clone()).collect();
        v.sort();
        v
    }

    /// Image files under the vault (png/jpg/gif/webp/svg/bmp), for the sidebar tree.
    pub fn list_image_rel_paths(&self) -> Vec<String> {
        let image_exts = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];
        let mut v = Vec::new();
        for entry in WalkDir::new(&self.root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !image_exts.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                continue;
            }
            let rel = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel.split('/').any(|p| p.starts_with('.')) {
                continue;
            }
            v.push(rel);
        }
        v.sort();
        v.dedup();
        v
    }

    /// Notes + images for the files sidebar.
    pub fn list_sidebar_rel_paths(&self) -> Vec<String> {
        let mut v = self.list_md_rel_paths();
        v.extend(self.list_image_rel_paths());
        v.sort();
        v.dedup();
        v
    }

    /// Create an empty folder under the vault root (`rel` uses `/` separators).
    pub fn create_folder(&mut self, rel: &str) -> Result<(), CoreError> {
        let rel = rel.trim_matches('/').replace('\\', "/");
        if rel.is_empty() {
            return Err(CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "empty folder path",
            )));
        }
        let path = self.root.join(&rel);
        std::fs::create_dir_all(&path)?;
        Ok(())
    }

    /// Rename a folder or note. `old_rel` / `new_rel` are vault-relative (`/` separators).
    /// For folders, all notes under the prefix are reindexed.
    pub fn rename_entry(&mut self, old_rel: &str, new_rel: &str) -> Result<(), CoreError> {
        let old_rel = normalize_vault_rel(old_rel);
        let new_rel = normalize_vault_rel(new_rel);
        if old_rel.is_empty() || new_rel.is_empty() || old_rel == new_rel {
            return Ok(());
        }
        if new_rel.contains("..") {
            return Err(CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid path",
            )));
        }
        let old_path = self.root.join(&old_rel);
        let new_path = self.root.join(&new_rel);
        if !old_path.exists() {
            return Err(CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing {old_rel}"),
            )));
        }
        // On case-insensitive filesystems `Note.md` "exists" when `note.md`
        // does; a rename that only changes case is still allowed.
        if new_path.exists() && !same_file(&old_path, &new_path) {
            return Err(CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{new_rel} already exists"),
            )));
        }
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&old_path, &new_path)?;

        let is_dir = new_path.is_dir();
        if is_dir {
            let prefix = format!("{old_rel}/");
            let affected: Vec<String> = self
                .index
                .notes()
                .filter(|n| n.rel == old_rel || n.rel.starts_with(&prefix))
                .map(|n| n.rel.clone())
                .collect();
            for rel in affected {
                let abs_old = self.root.join(&rel);
                self.index.remove_path(&self.root, &abs_old);
                let rest = rel.strip_prefix(&old_rel).unwrap_or("");
                let new_note_rel = format!("{new_rel}{rest}");
                let abs_new = self.root.join(&new_note_rel);
                if abs_new.exists() {
                    let _ = self.index.upsert_file(&self.root, &abs_new);
                }
            }
        } else {
            self.index.remove_path(&self.root, &old_path);
            if new_path.exists() {
                let _ = self.index.upsert_file(&self.root, &new_path);
            }
        }
        self.rebuild_graph();
        Ok(())
    }

    /// Delete a note file, or an empty folder.
    pub fn delete_entry(&mut self, rel: &str) -> Result<(), CoreError> {
        let rel = normalize_vault_rel(rel);
        let path = self.root.join(&rel);
        if path.is_dir() {
            let empty = std::fs::read_dir(&path)?.next().is_none();
            if !empty {
                return Err(CoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "folder is not empty",
                )));
            }
            std::fs::remove_dir(&path)?;
        } else if path.is_file() {
            std::fs::remove_file(&path)?;
            self.index.remove_path(&self.root, &path);
            self.rebuild_graph();
        } else {
            return Err(CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing {rel}"),
            )));
        }
        Ok(())
    }
}

fn normalize_vault_rel(rel: &str) -> String {
    rel.trim_matches('/').replace('\\', "/")
}

fn is_md_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// Do both paths name the same on-disk entry (e.g. differ only by case)?
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn rels(v: &Vault) -> Vec<String> {
        v.list_md_rel_paths()
    }

    #[test]
    fn external_folder_rename_updates_index_from_fs_event() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/one.md", "# One\n");
        write(dir.path(), "a/two.md", "# Two\n");
        write(dir.path(), "Home.md", "# Home\n");
        let mut v = Vault::open(dir.path()).unwrap();
        assert_eq!(rels(&v), vec!["Home.md", "a/one.md", "a/two.md"]);

        std::fs::rename(v.root.join("a"), v.root.join("b")).unwrap();
        // What the watcher reports: the old and the new directory path.
        assert!(v.apply_fs_path(&v.root.join("a")));
        assert!(
            !rels(&v).iter().any(|r| r.starts_with("a/")),
            "stale entries must go: {:?}",
            rels(&v)
        );
        assert!(v.apply_fs_path(&v.root.join("b")));
        assert_eq!(rels(&v), vec!["Home.md", "b/one.md", "b/two.md"]);
        assert!(v.index.get_by_rel("b/one.md").is_some());
    }

    #[test]
    fn fs_event_with_uppercase_md_extension_is_indexed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Home.md", "# Home\n");
        let mut v = Vault::open(dir.path()).unwrap();
        write(&v.root, "Caps.MD", "# Caps\n");
        assert!(v.apply_fs_path(&v.root.join("Caps.MD")));
        assert!(v.index.get_by_rel("Caps.MD").is_some());
    }

    #[test]
    fn watcher_delivers_external_folder_rename() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/one.md", "# One\n");
        let mut v = Vault::open(dir.path()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::rename(v.root.join("a"), v.root.join("b")).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            v.poll_fs();
            let ok = v.index.get_by_rel("b/one.md").is_some()
                && v.index.get_by_rel("a/one.md").is_none();
            if ok {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "index never caught up with the rename: {:?}",
                rels(&v)
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn rename_note_differing_only_by_case() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "note.md", "# note\n");
        let mut v = Vault::open(dir.path()).unwrap();
        v.rename_entry("note.md", "Note.md").expect("case-only rename must succeed");
        assert_eq!(rels(&v), vec!["Note.md"]);
        let on_disk: Vec<String> = std::fs::read_dir(&v.root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(on_disk.contains(&"Note.md".to_string()), "{on_disk:?}");
        assert!(!on_disk.contains(&"note.md".to_string()), "{on_disk:?}");
    }

    #[test]
    fn rename_onto_a_different_existing_note_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.md", "# a\n");
        write(dir.path(), "b.md", "# b\n");
        let mut v = Vault::open(dir.path()).unwrap();
        assert!(v.rename_entry("a.md", "b.md").is_err());
        assert_eq!(rels(&v), vec!["a.md", "b.md"]);
    }
}
