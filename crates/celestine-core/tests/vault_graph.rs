use celestine_core::{resolve_link, ResolvedLink, Vault};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

#[test]
fn fixture_vault_links_and_backlinks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vaults/minimal");
    let vault = Vault::open(&root).expect("open fixture");
    assert!(vault.index.len() >= 2);

    let home = vault.index.get_by_rel("Home.md").expect("Home.md");
    let alpha = vault.index.get_by_rel("Projects/Alpha.md").expect("Alpha");

    match resolve_link(&vault.index, &home.rel, "Projects/Alpha") {
        ResolvedLink::Note(id) => assert_eq!(id, alpha.id),
        other => panic!("expected note, got {other:?}"),
    }

    let backs = vault.graph.backlinks(&vault.index, home.id);
    assert!(backs.contains(&alpha.id), "Alpha should backlink Home");

    let edges = vault.graph.all_edges();
    assert!(
        edges
            .iter()
            .any(|(a, b)| (*a == home.id && *b == alpha.id) || (*a == alpha.id && *b == home.id)),
        "expected Home–Alpha edge"
    );
}

fn wait_for_refresh(vault: &mut Vault, wakes: &Receiver<()>, change: &str, ready: impl Fn(&Vault) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        wakes.recv_timeout(remaining).unwrap_or_else(|error| {
            panic!("timed out waiting for watcher wake after {change}: {error}");
        });
        if vault.poll_fs() && ready(vault) {
            return;
        }
        assert!(Instant::now() < deadline, "watcher did not refresh after {change}");
    }
}

#[test]
fn external_note_image_and_directory_changes_wake_and_refresh() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Folder/nested")).unwrap();
    std::fs::write(dir.path().join("Folder/nested/Child.md"), "[[Home]]").unwrap();
    std::fs::write(dir.path().join("Home.md"), "# Home").unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let mut vault = Vault::open_with_wakeup(dir.path(), move || {
        let _ = wake_tx.send(());
    })
    .unwrap();
    assert!(vault.watcher_error().is_none(), "{:?}", vault.watcher_error());
    assert!(vault.next_fs_poll().is_none(), "healthy watcher must stay timer-free");
    let root = vault.root.clone();
    let home_id = vault.index.get_by_rel("Home.md").unwrap().id;

    std::fs::write(root.join("New.md"), "[[Home]]").unwrap();
    wait_for_refresh(&mut vault, &wakes, "note creation", |vault| {
        vault.index.get_by_rel("New.md").is_some()
    });
    let new_id = vault.index.get_by_rel("New.md").unwrap().id;
    assert!(vault.graph.neighbors(home_id).contains(&new_id));

    std::fs::write(root.join("New.md"), "# Edited").unwrap();
    wait_for_refresh(&mut vault, &wakes, "note edit", |vault| {
        vault.index.get_by_rel("New.md").is_some_and(|note| note.content == "# Edited")
    });
    assert_eq!(vault.index.get_by_rel("New.md").unwrap().id, new_id);
    assert!(!vault.graph.neighbors(home_id).contains(&new_id));

    std::fs::rename(root.join("New.md"), root.join("Renamed.md")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "note rename", |vault| {
        vault.index.get_by_rel("New.md").is_none()
            && vault.index.get_by_rel("Renamed.md").is_some()
    });
    std::fs::remove_file(root.join("Renamed.md")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "note deletion", |vault| {
        vault.index.get_by_rel("Renamed.md").is_none()
    });

    std::fs::write(root.join("picture.png"), b"image inventory fixture").unwrap();
    wait_for_refresh(&mut vault, &wakes, "image creation", |vault| {
        vault.list_image_rel_paths() == ["picture.png"]
    });
    std::fs::rename(root.join("picture.png"), root.join("renamed.png")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "image rename", |vault| {
        vault.list_image_rel_paths() == ["renamed.png"]
    });
    std::fs::remove_file(root.join("renamed.png")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "image deletion", |vault| {
        vault.list_image_rel_paths().is_empty()
    });

    std::fs::rename(root.join("Folder"), root.join("Moved")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "directory rename", |vault| {
        vault.index.get_by_rel("Folder/nested/Child.md").is_none()
            && vault.index.get_by_rel("Moved/nested/Child.md").is_some()
    });
    std::fs::remove_dir_all(root.join("Moved")).unwrap();
    wait_for_refresh(&mut vault, &wakes, "directory deletion", |vault| {
        vault.list_md_rel_paths() == ["Home.md"]
    });
    assert_eq!(vault.index.get_by_rel("Home.md").unwrap().id, home_id);
    assert!(vault.graph.neighbors(home_id).is_empty());
}
