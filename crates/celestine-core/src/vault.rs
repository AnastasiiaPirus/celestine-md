use crate::graph::LinkGraph;
use crate::index::VaultIndex;
use crate::CoreError;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const FALLBACK_SCAN_INTERVAL: Duration = Duration::from_secs(2);

pub struct Vault {
    pub root: PathBuf,
    pub index: VaultIndex,
    pub graph: LinkGraph,
    _watcher: Option<RecommendedWatcher>,
    watcher_error: Option<notify::Error>,
    rx: Option<Receiver<Result<Event, notify::Error>>>,
    last_fs_reconcile: Instant,
}

impl Vault {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        Self::open_with_wakeup(root, || {})
    }

    /// Open a vault and wake its consumer after publishing filesystem changes.
    /// The callback runs on the watcher thread; indexing remains in `poll_fs`.
    pub fn open_with_wakeup(
        root: impl AsRef<Path>,
        wakeup: impl Fn() + Send + 'static,
    ) -> Result<Self, CoreError> {
        let root = root.as_ref().canonicalize().unwrap_or_else(|_| root.as_ref().to_path_buf());
        if !root.is_dir() {
            return Err(CoreError::NotADirectory(root));
        }
        let (tx, rx) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |result| publish_fs_event(result, &tx, &wakeup),
            notify::Config::default(),
        )
        .and_then(|mut watcher| {
            watcher.watch(&root, RecursiveMode::Recursive)?;
            Ok(watcher)
        });

        // Start watching before the initial scan so changes during it are queued.
        Ok(Self::from_watcher(root, watcher, rx))
    }

    fn from_watcher(
        root: PathBuf,
        watcher: notify::Result<RecommendedWatcher>,
        rx: Receiver<Result<Event, notify::Error>>,
    ) -> Self {
        let (watcher, watcher_error) = match watcher {
            Ok(watcher) => (Some(watcher), None),
            Err(error) => (None, Some(error)),
        };
        let mut vault = Self {
            root,
            index: VaultIndex::new(),
            graph: LinkGraph::default(),
            _watcher: watcher,
            watcher_error,
            rx: Some(rx),
            last_fs_reconcile: Instant::now(),
        };
        vault.reconcile_fs();
        vault
    }

    /// Startup or runtime watcher failures enable periodic disk scans.
    pub fn watcher_error(&self) -> Option<&notify::Error> {
        self.watcher_error.as_ref()
    }

    /// When the consumer must call `poll_fs` again without a watcher wakeup.
    /// A healthy watcher needs no periodic timer.
    pub fn next_fs_poll(&self) -> Option<Duration> {
        self.watcher_error.is_some().then(|| {
            FALLBACK_SCAN_INTERVAL.saturating_sub(self.last_fs_reconcile.elapsed())
        })
    }

    pub fn rebuild_graph(&mut self) {
        self.graph = LinkGraph::rebuild(&self.index);
    }

    fn reconcile_fs(&mut self) {
        let missing: Vec<_> = self
            .index
            .notes()
            .filter(|note| match std::fs::metadata(&note.path) {
                Ok(metadata) => !metadata.is_file(),
                Err(error) => error.kind() == std::io::ErrorKind::NotFound,
            })
            .map(|note| note.path.clone())
            .collect();
        for path in missing {
            self.index.remove_path(&self.root, &path);
        }
        for entry in WalkDir::new(&self.root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                let _ = self.index.upsert_file(&self.root, path);
            }
        }
        self.rebuild_graph();
        self.last_fs_reconcile = Instant::now();
    }

    /// Drain filesystem events and update the index. True also covers image and
    /// directory changes, which need a fresh sidebar even without note changes.
    pub fn poll_fs(&mut self) -> bool {
        let mut reconcile = self.next_fs_poll().is_some_and(|delay| delay.is_zero());
        let mut dirty = reconcile;
        let mut paths = HashSet::new();
        if let Some(rx) = &self.rx {
            while let Ok(result) = rx.try_recv() {
                match result {
                    Err(error) => {
                        // Some backend errors mean no further events will arrive.
                        // Keep a bounded fallback active after recovering this batch.
                        self.watcher_error = Some(error);
                        dirty = true;
                        reconcile = true;
                    }
                    Ok(event) => {
                        if event.need_rescan() {
                            dirty = true;
                            reconcile = true;
                        }
                        match event.kind {
                            EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                                // Rename pairs may be split or differ only in case.
                                // Reconcile using the spelling of actual disk entries.
                                dirty = true;
                                reconcile = true;
                            }
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                dirty = true;
                                paths.extend(event.paths);
                            }
                            EventKind::Any | EventKind::Other => {
                                dirty = true;
                                reconcile = true;
                            }
                            EventKind::Access(_) => {}
                        }
                    }
                }
            }
        }
        // Backends may report only the directory, including after it has gone.
        // Inspect indexed descendants as well as directories still on disk.
        reconcile |= paths.iter().any(|path| {
            path.is_dir()
                || self.index.notes().any(|note| note.path != *path && note.path.starts_with(path))
        });
        if reconcile {
            self.reconcile_fs();
        } else {
            let mut notes_changed = false;
            for path in paths {
                if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                    continue;
                }
                if path.is_file() {
                    let _ = self.index.upsert_file(&self.root, &path);
                } else {
                    self.index.remove_path(&self.root, &path);
                }
                notes_changed = true;
            }
            if notes_changed {
                self.rebuild_graph();
            }
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

fn publish_fs_event(
    result: Result<Event, notify::Error>,
    tx: &Sender<Result<Event, notify::Error>>,
    wakeup: &impl Fn(),
) {
    // Linux also reports our own directory/file reads. Discard access events
    // before enqueueing: waking on them loops, and queueing without waking leaks.
    if matches!(&result, Ok(event) if matches!(event.kind, EventKind::Access(_)) && !event.need_rescan()) {
        return;
    }
    if tx.send(result).is_ok() {
        wakeup();
    }
}

fn normalize_vault_rel(rel: &str) -> String {
    rel.trim_matches('/').replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode, CreateKind, Flag, ModifyKind, RemoveKind, RenameMode};
    use std::cell::Cell;

    // Inject backend notifications deterministically; disk reads, indexing and graph
    // updates still use the real implementation and a real temporary vault.
    fn queued_vault(root: &Path) -> (Vault, Sender<Result<Event, notify::Error>>) {
        let (tx, rx) = mpsc::channel();
        let mut vault = Vault {
            root: root.canonicalize().unwrap(),
            index: VaultIndex::new(),
            graph: LinkGraph::default(),
            _watcher: None,
            watcher_error: None,
            rx: Some(rx),
            last_fs_reconcile: Instant::now(),
        };
        vault.reconcile_fs();
        (vault, tx)
    }

    #[test]
    fn runtime_watcher_error_keeps_refreshing_later_unreported_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        assert!(vault.next_fs_poll().is_none(), "fixture needs a healthy watcher");
        tx.send(Err(notify::Error::generic("backend stopped"))).unwrap();
        assert!(vault.poll_fs());
        std::fs::write(vault.root.join("Unreported.md"), "# Changed after the error").unwrap();
        vault.last_fs_reconcile = Instant::now() - FALLBACK_SCAN_INTERVAL;

        assert!(vault.poll_fs(), "a backend error must enable later fallback scans");
        assert!(vault.index.get_by_rel("Unreported.md").is_some());
        assert!(vault.watcher_error().is_some());
    }

    #[test]
    fn read_notifications_are_neither_queued_nor_woken() {
        let (tx, rx) = mpsc::channel();
        let wakes = Cell::new(0);
        for kind in [
            AccessKind::Any,
            AccessKind::Read,
            AccessKind::Open(AccessMode::Read),
            AccessKind::Close(AccessMode::Read),
        ] {
            publish_fs_event(Ok(Event::new(EventKind::Access(kind))), &tx, &|| {
                wakes.set(wakes.get() + 1);
            });
        }
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert_eq!(wakes.get(), 0);
    }

    #[test]
    fn mutations_and_recovery_signals_are_published_before_wakeup() {
        let (tx, rx) = mpsc::channel();
        let wakes = Cell::new(0);
        for result in [
            Ok(Event::new(EventKind::Create(CreateKind::File))),
            Ok(Event::new(EventKind::Modify(ModifyKind::Any))),
            Ok(Event::new(EventKind::Remove(RemoveKind::Folder))),
            Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan)),
            Ok(Event::new(EventKind::Access(AccessKind::Any)).set_flag(Flag::Rescan)),
            Err(notify::Error::generic("backend error")),
        ] {
            publish_fs_event(result, &tx, &|| {
                assert!(rx.try_recv().is_ok(), "wake must observe the published event");
                wakes.set(wakes.get() + 1);
            });
        }
        assert_eq!(wakes.get(), 6);
        drop(rx);
        publish_fs_event(Ok(Event::new(EventKind::Any)), &tx, &|| {
            panic!("a closed vault must not wake its former consumer");
        });
    }

    #[test]
    fn unavailable_watcher_reconciles_disk_only_when_the_fallback_is_due() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (_tx, rx) = mpsc::channel();
        // Substitute only the OS watcher registration result. The fallback scan,
        // deadline, file IO, index and graph are the production implementation.
        let mut vault = Vault::from_watcher(
            root,
            Err(notify::Error::generic("watch registration failed")),
            rx,
        );
        assert!(vault.watcher_error().is_some());
        assert!(vault.next_fs_poll().is_some_and(|delay| !delay.is_zero()));
        std::fs::write(vault.root.join("Late.md"), "# Discovered by polling").unwrap();
        assert!(!vault.poll_fs(), "do not scan on every input frame");
        assert!(vault.index.is_empty());

        vault.last_fs_reconcile = Instant::now() - FALLBACK_SCAN_INTERVAL;
        assert!(vault.poll_fs());
        let note = vault.index.get_by_rel("Late.md").unwrap();
        assert_eq!(note.content, "# Discovered by polling");
        assert_eq!(vault.graph.node_ids(), [note.id]);
        assert!(vault.next_fs_poll().is_some_and(|delay| !delay.is_zero()));
        assert!(!vault.poll_fs(), "a scan must reset the fallback deadline");
    }

    #[test]
    fn image_event_requests_refresh_without_a_markdown_change() {
        let dir = tempfile::tempdir().unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let path = vault.root.join("picture.png");
        std::fs::write(&path, b"image bytes").unwrap();
        tx.send(Ok(Event::new(EventKind::Create(CreateKind::File)).add_path(path)))
            .unwrap();

        assert!(vault.poll_fs(), "image creation must refresh the sidebar");
        assert_eq!(vault.list_sidebar_rel_paths(), ["picture.png"]);
        assert!(!vault.poll_fs(), "a drained queue must become idle");
    }

    #[test]
    fn watcher_error_does_not_strand_the_following_change() {
        let dir = tempfile::tempdir().unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let path = vault.root.join("New.md");
        std::fs::write(&path, "# New\n").unwrap();
        tx.send(Err(notify::Error::generic("backend lost events")))
            .unwrap();
        tx.send(Ok(Event::new(EventKind::Create(CreateKind::File)).add_path(path)))
            .unwrap();

        assert!(vault.poll_fs(), "recover and consume the whole event batch");
        assert!(vault.index.get_by_rel("New.md").is_some());
        assert!(!vault.poll_fs(), "valid events after errors must be drained");
    }

    #[test]
    fn rescan_reconciles_disk_and_preserves_surviving_note_ids() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Keep.md"), "[[Old]]").unwrap();
        std::fs::write(dir.path().join("Old.md"), "# Old").unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let keep_id = vault.index.get_by_rel("Keep.md").unwrap().id;
        let old_id = vault.index.get_by_rel("Old.md").unwrap().id;
        std::fs::remove_file(vault.root.join("Old.md")).unwrap();
        std::fs::write(vault.root.join("New.md"), "[[Keep]]").unwrap();
        std::fs::write(vault.root.join("Keep.md"), "# Updated").unwrap();
        tx.send(Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan)))
            .unwrap();

        assert!(vault.poll_fs());
        assert_eq!(vault.list_md_rel_paths(), ["Keep.md", "New.md"]);
        assert_eq!(vault.index.get_by_rel("Keep.md").unwrap().id, keep_id);
        assert_eq!(vault.index.get(keep_id).unwrap().content, "# Updated");
        let new_id = vault.index.get_by_rel("New.md").unwrap().id;
        assert_ne!(new_id, old_id, "removed IDs must not be reused");
        assert_eq!(vault.graph.neighbors(keep_id), [new_id]);
    }

    #[test]
    fn directory_rename_reindexes_descendant_notes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Old/nested")).unwrap();
        std::fs::write(dir.path().join("Old/nested/Child.md"), "[[Home]]").unwrap();
        std::fs::write(dir.path().join("Home.md"), "# Home").unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let home_id = vault.index.get_by_rel("Home.md").unwrap().id;
        let old = vault.root.join("Old");
        let new = vault.root.join("New");
        std::fs::rename(&old, &new).unwrap();
        tx.send(Ok(Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(old)
            .add_path(new)))
            .unwrap();

        assert!(vault.poll_fs());
        assert_eq!(vault.list_md_rel_paths(), ["Home.md", "New/nested/Child.md"]);
        let child_id = vault.index.get_by_rel("New/nested/Child.md").unwrap().id;
        assert_eq!(vault.graph.neighbors(home_id), [child_id]);
    }

    #[test]
    fn directory_removal_drops_all_descendants_even_with_md_suffix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Folder.md/nested")).unwrap();
        std::fs::write(dir.path().join("Folder.md/nested/Child.md"), "# Child").unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let path = vault.root.join("Folder.md");
        std::fs::remove_dir_all(&path).unwrap();
        tx.send(Ok(Event::new(EventKind::Remove(RemoveKind::Folder)).add_path(path)))
            .unwrap();

        assert!(vault.poll_fs());
        assert!(vault.index.is_empty(), "deleted directory left stale notes");
        assert!(vault.graph.node_ids().is_empty());
    }

    #[test]
    fn case_only_rename_notifications_reconcile_actual_disk_spelling() {
        for mode in [RenameMode::From, RenameMode::Both] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("Anchor.md"), "[[Note]]").unwrap();
            std::fs::write(dir.path().join("note.md"), "# Renamed note").unwrap();
            let (mut vault, tx) = queued_vault(dir.path());
            let anchor_id = vault.index.get_by_rel("Anchor.md").unwrap().id;
            let old_id = vault.index.get_by_rel("note.md").unwrap().id;
            let old_path = vault.root.join("note.md");
            let new_path = vault.root.join("Note.md");
            std::fs::rename(&old_path, &new_path).unwrap();
            let mut event = Event::new(EventKind::Modify(ModifyKind::Name(mode)))
                .add_path(old_path);
            if mode == RenameMode::Both {
                event = event.add_path(new_path.clone());
            }
            tx.send(Ok(event)).unwrap();

            assert!(vault.poll_fs());
            assert_eq!(vault.list_md_rel_paths(), ["Anchor.md", "Note.md"], "{mode:?}");
            let renamed = vault.index.get_by_rel("Note.md").unwrap();
            assert_eq!(renamed.path, new_path);
            assert_eq!(vault.index.get_by_rel("Anchor.md").unwrap().id, anchor_id);
            assert_eq!(vault.graph.neighbors(anchor_id), [renamed.id]);
            if renamed.id != old_id {
                assert!(!vault.graph.node_ids().contains(&old_id));
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn rescan_retains_note_ids_when_a_directory_is_temporarily_unreadable() {
        use std::os::unix::fs::PermissionsExt;

        struct RestorePermissions {
            path: PathBuf,
            permissions: std::fs::Permissions,
        }
        impl Drop for RestorePermissions {
            fn drop(&mut self) {
                std::fs::set_permissions(&self.path, self.permissions.clone())
                    .expect("restore temporary test directory permissions");
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("Private");
        std::fs::create_dir(&private).unwrap();
        std::fs::write(private.join("Hidden.md"), "# Keep this note").unwrap();
        std::fs::write(dir.path().join("Anchor.md"), "[[Private/Hidden]]").unwrap();
        let (mut vault, tx) = queued_vault(dir.path());
        let anchor_id = vault.index.get_by_rel("Anchor.md").unwrap().id;
        let note_id = vault.index.get_by_rel("Private/Hidden.md").unwrap().id;
        let restore = RestorePermissions {
            path: private.clone(),
            permissions: std::fs::metadata(&private).unwrap().permissions(),
        };
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o0)).unwrap();
        match std::fs::metadata(private.join("Hidden.md")) {
            Ok(_) => {
                // Privileged users can bypass directory mode bits. No permission
                // failure was induced, so this environment cannot exercise it.
                eprintln!("skipping permission regression: metadata remained accessible");
                return;
            }
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied),
        }
        tx.send(Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan))).unwrap();

        assert!(vault.poll_fs());
        let retained = vault.index.get_by_rel("Private/Hidden.md")
            .expect("an inaccessible note must not be treated as deleted");
        assert_eq!(retained.id, note_id);
        assert_eq!(vault.graph.neighbors(anchor_id), [note_id]);

        drop(restore);
        std::fs::write(private.join("Hidden.md"), "# Accessible again").unwrap();
        tx.send(Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan))).unwrap();
        assert!(vault.poll_fs());
        let recovered = vault.index.get_by_rel("Private/Hidden.md").unwrap();
        assert_eq!(recovered.id, note_id);
        assert_eq!(recovered.content, "# Accessible again");
        assert_eq!(vault.graph.neighbors(anchor_id), [note_id]);
    }

}
