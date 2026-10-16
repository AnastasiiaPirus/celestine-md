use crate::file_tree;
use celestine_core::{NoteId, Vault, VaultIndex};
use egui::{pos2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::collections::HashMap;
use std::path::Path;

/// How long (seconds) the force layout runs after open / topology change.
const SETTLE_SECS: f32 = 0.85;

#[derive(Default)]
pub struct GraphState {
    pub positions: HashMap<NoteId, Pos2>,
    pub local_mode: bool,
    pub depth: usize,
    pub pan: Vec2,
    pub zoom: f32,
    initialized: bool,
    /// Remaining settle time; physics only runs while > 0.
    settle_t: f32,
    last_node_count: usize,
    last_local: bool,
    last_depth: usize,
    last_center: Option<NoteId>,
}

impl GraphState {
    pub fn new() -> Self {
        Self {
            depth: 2,
            zoom: 1.0,
            ..Default::default()
        }
    }

    /// Kick a fresh layout burst (e.g. when opening the graph view).
    pub fn restart_layout(&mut self) {
        self.initialized = false;
        self.settle_t = SETTLE_SECS;
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        vault: &Vault,
        center: Option<NoteId>,
        accent: Color32,
        muted: Color32,
        _text: Color32,
    ) -> Option<String> {
        let mut open = None;
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;

        if response.dragged() {
            self.pan += response.drag_delta();
        }
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.4, 3.0);
        }

        let nodes: Vec<NoteId> = if self.local_mode {
            if let Some(c) = center {
                vault.graph.local(c, self.depth).into_iter().collect()
            } else {
                vault.graph.node_ids()
            }
        } else {
            vault.graph.node_ids()
        };

        let topology_changed = !self.initialized
            || self.positions.len() != vault.index.len()
            || self.last_node_count != nodes.len()
            || self.last_local != self.local_mode
            || self.last_depth != self.depth
            || self.last_center != center;

        if topology_changed {
            layout_circle(&mut self.positions, &vault.index, &nodes, rect.center());
            self.initialized = true;
            self.settle_t = SETTLE_SECS;
            self.last_node_count = nodes.len();
            self.last_local = self.local_mode;
            self.last_depth = self.depth;
            self.last_center = center;
        }

        // Burst of force-directed motion, then freeze — no continuous drift.
        if self.settle_t > 0.0 {
            let dt = ui.input(|i| i.stable_dt).clamp(1.0 / 120.0, 1.0 / 30.0);
            let progress = 1.0 - (self.settle_t / SETTLE_SECS).clamp(0.0, 1.0);
            // Strong push early so nodes spread quickly, then ease to zero.
            let strength = (1.0 - progress).powf(1.4);
            settle(
                &mut self.positions,
                vault,
                &nodes,
                rect,
                dt * 2.2 * strength,
            );
            self.settle_t = (self.settle_t - dt).max(0.0);
            ui.ctx().request_repaint();
        }

        // Folder rainbow colors matching the file tree.
        let mut paths = vault.list_md_rel_paths();
        paths.sort();
        let tree = file_tree::build_tree(&paths);

        let edges = vault.graph.all_edges();
        for (a, b) in edges {
            if !nodes.contains(&a) || !nodes.contains(&b) {
                continue;
            }
            let Some(&pa) = self.positions.get(&a) else {
                continue;
            };
            let Some(&pb) = self.positions.get(&b) else {
                continue;
            };
            let pa = map(pa, rect.center(), self.pan, self.zoom);
            let pb = map(pb, rect.center(), self.pan, self.zoom);
            let edge_c = Color32::from_rgba_unmultiplied(muted.r(), muted.g(), muted.b(), 150);
            painter.line_segment([pa, pb], Stroke::new(1.0_f32, edge_c));
        }

        for id in &nodes {
            let Some(meta) = vault.index.get(*id) else {
                continue;
            };
            let Some(&p) = self.positions.get(id) else {
                continue;
            };
            let screen = map(p, rect.center(), self.pan, self.zoom);
            let deg = vault.graph.degree(*id).max(1) as f32;
            let r = (4.0 + deg.sqrt() * 3.0) * self.zoom;
            let is_center = center == Some(*id);
            let folder_c = file_tree::color_for_rel(&tree, &meta.rel);
            let fill = if is_center {
                accent
            } else {
                folder_c
            };
            painter.circle_filled(screen, r, fill);
            if is_center {
                painter.circle_stroke(screen, r + 2.0, Stroke::new(1.5_f32, accent));
            }
            painter.text(
                screen + Vec2::new(r + 4.0, -6.0),
                egui::Align2::LEFT_TOP,
                Path::new(&meta.rel)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| meta.rel.clone()),
                egui::FontId::monospace(11.0),
                muted,
            );
            let hit = Rect::from_center_size(screen, Vec2::splat(r * 2.5));
            if response.clicked()
                && hit.contains(response.interact_pointer_pos().unwrap_or(Pos2::ZERO))
            {
                open = Some(meta.rel.clone());
            }
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("── graph ──").monospace());
            if ui
                .selectable_label(!self.local_mode, "global")
                .clicked()
            {
                self.local_mode = false;
            }
            if ui.selectable_label(self.local_mode, "local").clicked() {
                self.local_mode = true;
            }
            if self.local_mode {
                ui.add(egui::Slider::new(&mut self.depth, 1..=4).text("depth"));
            }
            if self.settle_t > 0.0 {
                ui.label(
                    egui::RichText::new("settling…")
                        .monospace()
                        .size(11.0)
                        .color(muted),
                );
            }
        });

        open
    }
}

fn map(p: Pos2, center: Pos2, pan: Vec2, zoom: f32) -> Pos2 {
    center + (p.to_vec2() - center.to_vec2()) * zoom + pan
}

fn layout_circle(
    positions: &mut HashMap<NoteId, Pos2>,
    index: &VaultIndex,
    visible: &[NoteId],
    center: Pos2,
) {
    positions.retain(|id, _| index.get(*id).is_some());

    // Place any node that lacks a position onto a compact ring; settle expands it.
    let missing: Vec<NoteId> = visible
        .iter()
        .copied()
        .filter(|id| !positions.contains_key(id))
        .collect();
    if missing.is_empty() {
        return;
    }
    let n = missing.len() as f32;
    let radius = 48.0 + (visible.len() as f32).sqrt() * 12.0;
    for (i, id) in missing.iter().enumerate() {
        let ang = (i as f32) / n.max(1.0) * std::f32::consts::TAU + 0.15;
        positions.insert(
            *id,
            pos2(
                center.x + radius * ang.cos(),
                center.y + radius * ang.sin(),
            ),
        );
    }
}

fn settle(
    positions: &mut HashMap<NoteId, Pos2>,
    vault: &Vault,
    visible: &[NoteId],
    view: Rect,
    dt: f32,
) {
    if dt <= 0.0 || visible.is_empty() {
        return;
    }
    let ids = visible;
    let mut forces: HashMap<NoteId, Vec2> = HashMap::new();
    for &id in ids {
        forces.insert(id, Vec2::ZERO);
    }

    // Soft pull toward view center so the cloud stays on screen.
    let center = view.center();
    for &id in ids {
        let Some(&p) = positions.get(&id) else {
            continue;
        };
        let to_c = center - p;
        *forces.get_mut(&id).unwrap() += to_c * 0.08 * dt;
    }

    // Repulsion between visible nodes
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let a = ids[i];
            let b = ids[j];
            let pa = positions[&a];
            let pb = positions[&b];
            let mut d = pa - pb;
            let dist = d.length().max(12.0);
            d /= dist;
            let f = d * (14_000.0 / (dist * dist)) * dt;
            *forces.get_mut(&a).unwrap() += f;
            *forces.get_mut(&b).unwrap() -= f;
        }
    }

    // Attraction along edges (visible endpoints only)
    for (a, b) in vault.graph.all_edges() {
        if !forces.contains_key(&a) || !forces.contains_key(&b) {
            continue;
        }
        let Some(&pa) = positions.get(&a) else {
            continue;
        };
        let Some(&pb) = positions.get(&b) else {
            continue;
        };
        let mut d = pb - pa;
        let dist = d.length().max(1.0);
        d /= dist;
        let ideal = 100.0;
        let f = d * (dist - ideal) * 0.04 * dt;
        if let Some(fa) = forces.get_mut(&a) {
            *fa += f;
        }
        if let Some(fb) = forces.get_mut(&b) {
            *fb -= f;
        }
    }

    let max_step = 28.0 * dt.max(0.05);
    for (id, f) in forces {
        if let Some(p) = positions.get_mut(&id) {
            let step = if f.length() > max_step {
                f.normalized() * max_step
            } else {
                f
            };
            *p += step;
            // Keep inside a padded view box in world space (pre-zoom).
            let pad = 40.0;
            p.x = p.x.clamp(view.left() + pad, view.right() - pad);
            p.y = p.y.clamp(view.top() + pad, view.bottom() - pad);
        }
    }
}
