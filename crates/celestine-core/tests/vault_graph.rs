use celestine_core::{resolve_link, ResolvedLink, Vault};
use std::path::PathBuf;

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
