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
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
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
        let Some(rx) = &self.rx else {
            return false;
        };
        let mut dirty = false;
        while let Ok(Ok(event)) = rx.try_recv() {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                    for path in event.paths {
                        if path.extension().and_then(|e| e.to_str()) != Some("md") {
                            continue;
                        }
                        if path.exists() {
                            let _ = self.index.upsert_file(&self.root, &path);
                        } else {
                            self.index.remove_path(&self.root, &path);
                        }
                        dirty = true;
                    }
                }
                _ => {}
            }
        }
        if dirty {
            self.rebuild_graph();
        }
        dirty
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
        if new_path.exists() {
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
