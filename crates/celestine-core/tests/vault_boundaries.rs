use celestine_core::{Vault, VaultIndex};
use std::fs;
use tempfile::tempdir;

#[test]
fn read_write_and_create_reject_paths_outside_the_vault() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside.md");
    fs::create_dir(&root).unwrap();
    fs::write(&outside, "sentinel").unwrap();
    let mut vault = Vault::open(&root).unwrap();

    assert!(vault.read_note_content("../outside.md").is_err());
    assert!(vault.write_note("../outside.md", "changed").is_err());
    assert!(vault.create_note("../outside-new").is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "sentinel");
    assert!(!temp.path().join("outside-new.md").exists());
    assert!(!temp.path().join("outside-new").exists());

    let absolute = temp.path().join("absolute.md");
    fs::write(&absolute, "absolute sentinel").unwrap();
    assert!(vault.read_note_content(absolute.to_str().unwrap()).is_err());
    assert!(vault.write_note(absolute.to_str().unwrap(), "changed").is_err());
    assert!(vault.create_note(absolute.to_str().unwrap()).is_err());
    assert_eq!(fs::read_to_string(&absolute).unwrap(), "absolute sentinel");
}

#[test]
fn folder_and_entry_mutations_reject_outside_paths_without_side_effects() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside_dir = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside_dir).unwrap();
    fs::write(outside_dir.join("source.md"), "source sentinel").unwrap();
    fs::write(outside_dir.join("target.md"), "target sentinel").unwrap();
    let mut vault = Vault::open(&root).unwrap();
    fs::write(root.join("inside.md"), "inside sentinel").unwrap();

    assert!(vault.create_folder("../outside/new/deep").is_err());
    assert!(!outside_dir.join("new").exists());
    assert!(vault.delete_entry("../outside/target.md").is_err());
    assert_eq!(fs::read_to_string(outside_dir.join("target.md")).unwrap(), "target sentinel");
    assert!(vault.rename_entry("../outside/source.md", "inside.md").is_err());
    assert!(vault.rename_entry("inside.md", "../outside/renamed.md").is_err());
    assert_eq!(fs::read_to_string(outside_dir.join("source.md")).unwrap(), "source sentinel");
    assert_eq!(fs::read_to_string(root.join("inside.md")).unwrap(), "inside sentinel");
    assert!(!outside_dir.join("renamed.md").exists());

    let absolute_target = temp.path().join("absolute-folder");
    assert!(vault.create_folder(absolute_target.to_str().unwrap()).is_err());
    assert!(!absolute_target.exists());
}

#[test]
fn nested_note_operations_and_inside_parent_navigation_work() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("note.md"), "before").unwrap();
    let mut vault = Vault::open(&root).unwrap();

    vault.write_note("Projects/../note.md", "after").unwrap();
    assert_eq!(fs::read_to_string(root.join("note.md")).unwrap(), "after");
    vault.create_note("Projects/Nested/New").unwrap();
    assert!(root.join("Projects/Nested/New.md").is_file());
    vault.create_folder("Projects/Nested/Empty").unwrap();
    vault.rename_entry("Projects/Nested/New.md", "Projects/Nested/Renamed.md").unwrap();
    assert!(root.join("Projects/Nested/Renamed.md").is_file());
    vault.delete_entry("Projects/Nested/Renamed.md").unwrap();
    assert!(!root.join("Projects/Nested/Renamed.md").exists());
}

#[cfg(unix)]
#[test]
fn symlinks_to_outside_files_and_directories_are_rejected() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel.md"), "sentinel").unwrap();
    symlink(outside.join("sentinel.md"), root.join("file-link.md")).unwrap();
    symlink(&outside, root.join("dir-link")).unwrap();
    let mut vault = Vault::open(&root).unwrap();

    assert!(vault.read_note_content("file-link.md").is_err());
    assert!(vault.write_note("file-link.md", "changed").is_err());
    assert!(vault.create_note("dir-link/new/note").is_err());
    assert!(vault.write_note("dir-link/new/note.md", "changed").is_err());
    assert!(vault.create_folder("dir-link/new-folder").is_err());
    assert_eq!(fs::read_to_string(outside.join("sentinel.md")).unwrap(), "sentinel");
    assert!(!outside.join("new").exists());
    assert!(!outside.join("new-folder").exists());
}

#[cfg(unix)]
#[test]
fn dangling_symlinks_cannot_create_files_outside_the_vault() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    symlink(outside.join("created-by-create.md"), root.join("create-link.md")).unwrap();
    symlink(outside.join("created-by-write.md"), root.join("write-link.md")).unwrap();
    let mut vault = Vault::open(&root).unwrap();

    assert!(vault.create_note("create-link.md").is_err());
    assert!(vault.write_note("write-link.md", "outside").is_err());
    assert!(!outside.join("created-by-create.md").exists());
    assert!(!outside.join("created-by-write.md").exists());
}

#[cfg(unix)]
#[test]
fn symlinks_that_resolve_inside_the_vault_remain_usable() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("real")).unwrap();
    fs::write(root.join("real/note.md"), "before").unwrap();
    symlink(root.join("real/note.md"), root.join("note-link.md")).unwrap();
    symlink(root.join("real"), root.join("dir-link")).unwrap();
    let mut vault = Vault::open(&root).unwrap();

    assert_eq!(vault.read_note_content("note-link.md").unwrap(), "before");
    vault.write_note("note-link.md", "after").unwrap();
    vault.create_note("dir-link/new").unwrap();
    vault.create_folder("dir-link/child").unwrap();
    assert_eq!(fs::read_to_string(root.join("real/note.md")).unwrap(), "after");
    assert!(root.join("real/new.md").is_file());
    assert!(root.join("real/child").is_dir());
}

#[test]
fn index_upsert_rejects_an_absolute_path_outside_the_vault() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside.md");
    fs::create_dir(&root).unwrap();
    fs::write(&outside, "# Outside\n").unwrap();
    let mut index = VaultIndex::new();

    assert!(index.upsert_file(&root, &outside).is_err());
    assert!(index.is_empty());
}

#[cfg(unix)]
#[test]
fn index_upsert_rejects_in_vault_symlink_to_outside_markdown() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside.md");
    fs::create_dir(&root).unwrap();
    fs::write(&outside, "# Outside sentinel\n").unwrap();
    symlink(&outside, root.join("linked.md")).unwrap();
    let mut index = VaultIndex::new();

    assert!(index.upsert_file(&root, &root.join("linked.md")).is_err());
    assert!(index.is_empty());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "# Outside sentinel\n");
}

#[test]
fn windows_path_spellings_are_rejected_and_nested_backslashes_stay_in_vault() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("vault");
    let outside = temp.path().join("outside.md");
    fs::create_dir(&root).unwrap();
    fs::write(&outside, "sentinel").unwrap();
    let mut vault = Vault::open(&root).unwrap();

    for path in [
        "C:\\outside.md",
        "C:outside.md",
        "\\\\server\\share\\outside.md",
        "..\\outside.md",
    ] {
        assert!(vault.write_note(path, "changed").is_err(), "accepted {path:?}");
    }
    assert_eq!(fs::read_to_string(&outside).unwrap(), "sentinel");

    vault.write_note("nested\\safe.md", "inside").unwrap();
    assert_eq!(fs::read_to_string(root.join("nested/safe.md")).unwrap(), "inside");
}
