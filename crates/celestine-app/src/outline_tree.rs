use egui::{Label, RichText, Sense, Ui};
use std::collections::HashSet;

/// Match file-tree sidebar size (~25% larger than the previous 12.5px).
pub const OUTLINE_FONT: f32 = 15.6;
const OUTLINE_ICON: f32 = 13.0;

#[derive(Clone)]
pub struct OutlineNode {
    pub text: String,
    pub line: usize,
    pub level: u8,
    pub children: Vec<OutlineNode>,
}

/// Nest flat ATX headings into a file-tree-like hierarchy.
pub fn build(headings: &[(u8, String, usize)]) -> Vec<OutlineNode> {
    let mut roots: Vec<OutlineNode> = Vec::new();
    let mut stack: Vec<OutlineNode> = Vec::new();

    for &(level, ref text, line) in headings {
        let node = OutlineNode {
            text: text.clone(),
            line,
            level,
            children: Vec::new(),
        };
        while stack.last().is_some_and(|top| top.level >= level) {
            let finished = stack.pop().unwrap();
            if let Some(parent) = stack.last_mut() {
                parent.children.push(finished);
            } else {
                roots.push(finished);
            }
        }
        stack.push(node);
    }
    while let Some(finished) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(finished);
        } else {
            roots.push(finished);
        }
    }
    roots
}

/// Draw outline as a collapsible scope tree (no `#` markers).
/// Returns `(line, heading_text)` when a heading is clicked.
pub fn show(
    ui: &mut Ui,
    nodes: &[OutlineNode],
    collapsed: &mut HashSet<String>,
    accent: egui::Color32,
    muted: egui::Color32,
    text: egui::Color32,
) -> Option<(usize, String)> {
    show_nodes(ui, nodes, "", collapsed, accent, muted, text)
}

fn show_nodes(
    ui: &mut Ui,
    nodes: &[OutlineNode],
    prefix: &str,
    collapsed: &mut HashSet<String>,
    accent: egui::Color32,
    muted: egui::Color32,
    text: egui::Color32,
) -> Option<(usize, String)> {
    let mut jump = None;
    for (i, node) in nodes.iter().enumerate() {
        let path = if prefix.is_empty() {
            format!("{i}")
        } else {
            format!("{prefix}/{i}")
        };
        let has_kids = !node.children.is_empty();
        let is_open = has_kids && !collapsed.contains(&path);
        let color = match node.level {
            1 => accent,
            2 => egui::Color32::from_rgb(0x7a, 0xc2, 0xf0),
            3 => egui::Color32::from_rgb(0xb4, 0x9d, 0xeb),
            4 => egui::Color32::from_rgb(0x9e, 0xce, 0x6a),
            5 => egui::Color32::from_rgb(0xe0, 0xaf, 0x68),
            _ => text,
        };

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.set_max_width(ui.available_width());

            if has_kids {
                let mark = if is_open {
                    "chevron-down"
                } else {
                    "chevron-right"
                };
                if crate::icons::button(ui, mark, OUTLINE_ICON, muted).clicked() {
                    if is_open {
                        collapsed.insert(path.clone());
                    } else {
                        collapsed.remove(&path);
                    }
                }
            } else {
                ui.add_space(OUTLINE_ICON + 4.0);
            }

            let title_w = ui.available_width().max(40.0);
            ui.allocate_ui_with_layout(
                egui::vec2(title_w, 0.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_max_width(title_w);
                    let label = RichText::new(&node.text).size(OUTLINE_FONT).color(color);
                    let resp = ui.add(Label::new(label).wrap().sense(Sense::click()));
                    if resp.clicked() {
                        jump = Some((node.line, node.text.clone()));
                    }
                },
            );
        });

        if has_kids && is_open {
            ui.indent(format!("outline-{path}"), |ui| {
                ui.set_max_width(ui.available_width());
                if let Some(j) =
                    show_nodes(ui, &node.children, &path, collapsed, accent, muted, text)
                {
                    jump = Some(j);
                }
            });
        }
    }
    jump
}
