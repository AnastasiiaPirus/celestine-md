use egui::{Color32, RichText, Ui};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

#[derive(Default)]
pub struct DirNode {
    dirs: BTreeMap<String, DirNode>,
    files: BTreeMap<String, String>, // name -> full rel path
}

fn insert_path(root: &mut DirNode, rel: &str) {
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut node = root;
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            node.files.insert((*part).to_string(), rel.to_string());
        } else {
            node = node
                .dirs
                .entry((*part).to_string())
                .or_insert_with(DirNode::default);
        }
    }
}

pub fn build_tree(paths: &[String]) -> DirNode {
    let mut root = DirNode::default();
    for p in paths {
        insert_path(&mut root, p);
    }
    root
}

/// Built-in Lucide/flea marks for folders (right-click a folder to assign).
pub use crate::icons::FOLDER_ICON_CHOICES;

/// Sidebar tree typography (~25% larger than the old 12.5px).
pub const TREE_FONT: f32 = 15.6;
const TREE_ICON: f32 = 14.0;

/// Sibling-index rainbow (reference style): each child of a folder cycles the
/// palette; nesting does not shift hue — only the index among siblings.
pub fn folder_rainbow(sibling_index: usize) -> Color32 {
    const PALETTE: &[Color32] = &[
        Color32::from_rgb(0xe8, 0x7a, 0x8a), // salmon / pink
        Color32::from_rgb(0xe8, 0xa0, 0x5c), // orange
        Color32::from_rgb(0xd4, 0xb8, 0x4a), // golden yellow
        Color32::from_rgb(0x7a, 0xc4, 0x6e), // leaf green
        Color32::from_rgb(0x6a, 0xb4, 0xd8), // sky blue
        Color32::from_rgb(0x9a, 0x8a, 0xd8), // violet
        Color32::from_rgb(0xd0, 0x6a, 0xb8), // magenta
        Color32::from_rgb(0xb8, 0x9a, 0x6e), // tan / brown
        Color32::from_rgb(0x9a, 0x9a, 0xa0), // gray
        Color32::from_rgb(0xd0, 0x5c, 0x5c), // deep red
    ];
    PALETTE[sibling_index % PALETTE.len()]
}

/// Color for notes at the vault root (no parent folder).
pub const ROOT_NOTE_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb8);

/// Rainbow color of a folder path (last component's sibling index under its parent).
pub fn color_for_folder(root: &DirNode, folder_path: &str) -> Option<Color32> {
    let parts: Vec<&str> = folder_path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut node = root;
    let mut color = None;
    for part in parts {
        let idx = node.dirs.keys().position(|k| k == part)?;
        color = Some(folder_rainbow(idx));
        node = node.dirs.get(part)?;
    }
    color
}

/// Color a note by the rainbow of its immediate parent folder.
pub fn color_for_rel(root: &DirNode, rel: &str) -> Color32 {
    let parent = Path::new(rel).parent().and_then(|p| {
        let s = p.to_string_lossy();
        if s.is_empty() || s == "." {
            None
        } else {
            Some(s.replace('\\', "/"))
        }
    });
    match parent {
        Some(folder) => color_for_folder(root, &folder).unwrap_or(ROOT_NOTE_COLOR),
        None => ROOT_NOTE_COLOR,
    }
}

pub struct TreeOutcome {
    pub open_rel: Option<String>,
    /// Folder path whose icon was changed (caller should persist).
    pub icons_changed: bool,
}

/// Draw a collapsible file tree with rainbow folders + custom icons.
pub fn show_tree(
    ui: &mut Ui,
    root: &DirNode,
    expanded: &mut HashSet<String>,
    folder_icons: &mut HashMap<String, String>,
    accent: Color32,
    muted: Color32,
    active_rel: Option<&str>,
) -> TreeOutcome {
    let active_ancestors = active_ancestors(active_rel);
    let mut outcome = TreeOutcome {
        open_rel: None,
        icons_changed: false,
    };
    show_dir(
        ui,
        root,
        "",
        ROOT_NOTE_COLOR, // vault-root notes share this; folders below get their own rainbow
        expanded,
        folder_icons,
        accent,
        muted,
        active_rel,
        &active_ancestors,
        &mut outcome,
    );
    outcome
}

fn active_ancestors(active_rel: Option<&str>) -> HashSet<String> {
    let mut set = HashSet::new();
    let Some(rel) = active_rel else {
        return set;
    };
    let mut acc = String::new();
    if let Some(parent) = Path::new(rel).parent() {
        for part in parent.components() {
            let p = part.as_os_str().to_string_lossy();
            if p == "." || p.is_empty() {
                continue;
            }
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(&p);
            set.insert(acc.clone());
        }
    }
    set
}

fn show_dir(
    ui: &mut Ui,
    node: &DirNode,
    prefix: &str,
    // Color of this directory — files listed here inherit it; child folders pick their own.
    folder_color: Color32,
    expanded: &mut HashSet<String>,
    folder_icons: &mut HashMap<String, String>,
    accent: Color32,
    muted: Color32,
    active_rel: Option<&str>,
    active_ancestors: &HashSet<String>,
    outcome: &mut TreeOutcome,
) {
    for (sibling_i, (name, child)) in node.dirs.iter().enumerate() {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let is_open = expanded.contains(&path);
        let on_path = active_ancestors.contains(&path);
        // Child folders cycle the rainbow among siblings (not inherited from parent).
        let own_color = folder_rainbow(sibling_i);
        let mut color = own_color;
        if on_path {
            // Brighten ancestors of the open note so the active branch pops.
            color = brighten(color, 1.18);
        }
        let custom = folder_icons.get(&path).map(|s| s.as_str());
        let folder_mark = match custom {
            Some(c) => crate::icons::normalize_icon_name(c),
            None if is_open => "folder-open",
            None => "folder",
        };
        let chevron = if is_open {
            "chevron-down"
        } else {
            "chevron-right"
        };

        let row_h = TREE_FONT + 6.0;
        let row_w = ui.available_width();
        let (row_rect, resp) = ui.allocate_exact_size(
            egui::vec2(row_w, row_h),
            egui::Sense::click(),
        );
        if on_path {
            ui.painter().rect_filled(
                row_rect,
                2.0,
                color.gamma_multiply(0.18),
            );
        }
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(row_h);
                ui.spacing_mut().item_spacing.x = 4.0;
                crate::icons::show(ui, chevron, TREE_ICON * 0.85, color);
                crate::icons::show(ui, folder_mark, TREE_ICON, color);
                ui.label(RichText::new(name).size(TREE_FONT).color(color).strong());
            });
        });
        if resp.clicked() {
            if is_open {
                expanded.remove(&path);
            } else {
                expanded.insert(path.clone());
            }
        }
        resp.context_menu(|ui| {
            ui.label(RichText::new("folder mark").size(11.0).color(muted));
            ui.add_space(4.0);
            egui::Grid::new(format!("icon-grid-{path}"))
                .num_columns(6)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    for (i, mark) in FOLDER_ICON_CHOICES.iter().enumerate() {
                        let r = crate::icons::button(ui, mark, 18.0, color);
                        if r.clicked() {
                            folder_icons
                                .insert(path.clone(), (*mark).to_string());
                            outcome.icons_changed = true;
                            ui.close_menu();
                        }
                        r.on_hover_text(*mark);
                        if (i + 1) % 6 == 0 {
                            ui.end_row();
                        }
                    }
                    ui.end_row();
                });
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("reset mark").size(12.0).color(muted))
                        .frame(false),
                )
                .clicked()
            {
                folder_icons.remove(&path);
                outcome.icons_changed = true;
                ui.close_menu();
            }
        });

        if expanded.contains(&path) {
            ui.indent(format!("dir-{path}"), |ui| {
                // Notes inside this folder take its rainbow; nested folders get new sibling hues.
                show_dir(
                    ui,
                    child,
                    &path,
                    own_color,
                    expanded,
                    folder_icons,
                    accent,
                    muted,
                    active_rel,
                    active_ancestors,
                    outcome,
                );
            });
        }
    }

    for (name, rel) in &node.files {
        let selected = active_rel == Some(rel.as_str());
        // Files match the enclosing folder's color (latest parent), not muted/accent.
        let mut color = folder_color;
        if selected {
            color = brighten(color, 1.2);
        }
        let row_h = TREE_FONT + 6.0;
        let row_w = ui.available_width();
        let (row_rect, resp) = ui.allocate_exact_size(
            egui::vec2(row_w, row_h),
            egui::Sense::click(),
        );
        if selected {
            ui.painter().rect_filled(
                row_rect,
                2.0,
                folder_color.gamma_multiply(0.22),
            );
        }
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(row_h);
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(TREE_ICON * 0.85 + 4.0);
                crate::icons::show(ui, "file-text", TREE_ICON, color);
                let mut label = RichText::new(name).size(TREE_FONT).color(color);
                if selected {
                    label = label.strong();
                }
                ui.label(label);
            });
        });
        if resp.clicked() {
            outcome.open_rel = Some(rel.clone());
        }
    }
}

fn brighten(c: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        ((c.r() as f32 * factor).min(255.0)) as u8,
        ((c.g() as f32 * factor).min(255.0)) as u8,
        ((c.b() as f32 * factor).min(255.0)) as u8,
    )
}

/// Load/save per-vault folder icon map from `.celestine/folder-icons.toml`.
pub fn load_folder_icons(vault_root: &Path) -> HashMap<String, String> {
    let path = vault_root.join(".celestine").join("folder-icons.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    #[derive(serde::Deserialize, Default)]
    struct File {
        #[serde(default)]
        icons: HashMap<String, String>,
    }
    let mut icons = toml::from_str::<File>(&text)
        .map(|f| f.icons)
        .unwrap_or_default();
    // Migrate legacy emoji assignments → Lucide/flea mark names.
    for v in icons.values_mut() {
        *v = crate::icons::normalize_icon_name(v).to_string();
    }
    icons
}

pub fn save_folder_icons(vault_root: &Path, icons: &HashMap<String, String>) {
    let dir = vault_root.join(".celestine");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("folder-icons.toml");
    let mut map = BTreeMap::new();
    for (k, v) in icons {
        map.insert(k.clone(), v.clone());
    }
    let body = match toml::to_string_pretty(&FolderIconsFile { icons: map }) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = std::fs::write(path, body);
}

#[derive(serde::Serialize)]
struct FolderIconsFile {
    icons: BTreeMap<String, String>,
}
