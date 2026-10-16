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
}
