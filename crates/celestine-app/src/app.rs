use crate::file_tree;
use crate::graph_view::GraphState;
use crate::keybinds::{ensure_user_config, Command, Keymap};
use crate::md_edit;
use crate::outline_tree;
use crate::preview;
use crate::state::AppState;
use crate::theme_apply::{self, color, naked_button, naked_icon, sidebar_toggle};
use celestine_core::{extract_note_data, NoteId, Vault};
use celestine_theme::{list_available_themes, load_theme, ThemeTokens};
use egui::text::{CCursor, CCursorRange};
use egui::{FontId, RichText, ScrollArea, Stroke, TextEdit, Ui};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    QuickOpen,
    CommandPalette,
    VaultSearch,
    Shortcuts,
    Settings,
    Find,
    ThemePicker,
    RecentVaults,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Files,
    Editor,
    Outline,
    Graph,
}

struct OpenTab {
    rel: String,
    buffer: String,
    dirty: bool,
    preview_mode: bool,
    /// Jump outline → editor/preview
    scroll_to_line: Option<usize>,
    scroll_to_heading: Option<String>,
    editor_text_id: egui::Id,
}

pub struct CelestineApp {
    vault: Option<Vault>,
    tabs: Vec<OpenTab>,
    active_tab: usize,
    former_tab: usize,
    keymap: Keymap,
    theme: ThemeTokens,
    theme_name: String,
    overlay: Overlay,
    overlay_query: String,
    overlay_selected: usize,
    overlay_anim: f32,
    focus: FocusPane,
    show_graph: bool,
    graph: GraphState,
    status: String,
    nav_back: Vec<String>,
    nav_forward: Vec<String>,
    find_query: String,
    editor_focused: bool,
    last_frame: Instant,
    persist: AppState,
    tree_expanded: HashSet<String>,
    /// Outline paths that are collapsed (default = expanded).
    outline_collapsed: HashSet<String>,
    /// One-shot: grab keyboard focus into the overlay filter on open.
    overlay_focus_pending: bool,
    /// Per-vault custom folder icons (rel path → emoji).
    folder_icons: HashMap<String, String>,
}

impl CelestineApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ensure_user_config();
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut persist = AppState::load();
        persist.clear_recent_missing();

        let theme_name = std::env::var("CELESTINE_THEME")
            .ok()
            .or_else(|| persist.theme.clone())
            .unwrap_or_else(|| {
                if crate::keybinds::is_omarchy() {
                    "terminal".into()
                } else {
                    "obsidian-dark".into()
                }
            });
        let theme = load_theme(&theme_name).unwrap_or_default();
        crate::fonts::install(&cc.egui_ctx);
        theme_apply::apply_theme(&cc.egui_ctx, &theme, persist.ui_font_scale);

        let mut app = Self {
            vault: None,
            tabs: Vec::new(),
            active_tab: 0,
            former_tab: 0,
            keymap: Keymap::detect_and_load(),
            theme,
            theme_name,
            overlay: Overlay::None,
            overlay_query: String::new(),
            overlay_selected: 0,
            overlay_anim: 0.0,
            focus: FocusPane::Editor,
            show_graph: false,
            graph: GraphState::new(),
            status: format!(
                "open vault {} · recent {} · themes {} · ?",
                "Mod+Shift+O", "Mod+R", "Mod+T"
            ),
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            find_query: String::new(),
            editor_focused: false,
            last_frame: Instant::now(),
            persist,
            tree_expanded: HashSet::new(),
            outline_collapsed: HashSet::new(),
            overlay_focus_pending: false,
            folder_icons: HashMap::new(),
        };

        if let Some(path) = std::env::args().nth(1) {
            app.open_vault(PathBuf::from(path), &cc.egui_ctx);
        } else if let Some(path) = app.persist.vault_to_restore() {
            app.open_vault(path, &cc.egui_ctx);
        }
        app
    }

    fn open_vault(&mut self, path: PathBuf, ctx: &egui::Context) {
        match Vault::open(&path) {
            Ok(v) => {
                self.status = format!("vault {}", path.display());
                self.tabs.clear();
                self.active_tab = 0;
                self.show_graph = false;
                let first = v
                    .list_md_rel_paths()
                    .into_iter()
                    .find(|r| r.eq_ignore_ascii_case("Home.md"))
                    .or_else(|| v.list_md_rel_paths().into_iter().next());
                self.folder_icons = file_tree::load_folder_icons(&path);
                self.vault = Some(v);
                self.persist.push_recent(&path);
                if let Some(rel) = first {
                    self.open_note(&rel, false);
                }
                ctx.request_repaint();
            }
            Err(e) => self.status = format!("failed to open vault: {e}"),
        }
    }

    fn close_vault(&mut self) {
        self.vault = None;
        self.tabs.clear();
        self.active_tab = 0;
        self.former_tab = 0;
        self.show_graph = false;
        self.nav_back.clear();
        self.nav_forward.clear();
        self.folder_icons.clear();
        // Keep last_vault so the next launch still restores; clear only the live session.
        self.status = "vault closed — open vault · recent vaults".into();
    }

    fn persist_folder_icons(&self) {
        if let Some(vault) = &self.vault {
            file_tree::save_folder_icons(&vault.root, &self.folder_icons);
        }
    }

    fn toggle_left_sidebar(&mut self) {
        self.persist.show_left_sidebar = !self.persist.show_left_sidebar;
        self.persist.save();
    }

    fn toggle_right_sidebar(&mut self) {
        self.persist.show_right_sidebar = !self.persist.show_right_sidebar;
        self.persist.save();
    }

    fn set_theme(&mut self, name: &str, ctx: &egui::Context) {
        if let Ok(t) = load_theme(name) {
            self.theme_name = name.to_string();
            self.theme = t;
            self.persist.theme = Some(name.to_string());
            self.persist.save();
            theme_apply::apply_theme(ctx, &self.theme, self.persist.ui_font_scale);
            self.status = format!("theme {name}");
        }
    }

    fn bump_editor_font(&mut self, delta: f32) {
        self.persist.editor_font_size =
            (self.persist.editor_font_size + delta).clamp(10.0, 36.0);
        self.persist.save();
        self.status = format!("editor font {:.0}px", self.persist.editor_font_size);
    }

    fn bump_ui_font(&mut self, delta: f32, ctx: &egui::Context) {
        self.persist.ui_font_scale =
            (self.persist.ui_font_scale + delta).clamp(0.85, 1.6);
        self.persist.save();
        theme_apply::apply_theme(ctx, &self.theme, self.persist.ui_font_scale);
        self.status = format!(
            "UI font {:.0}%",
            self.persist.ui_font_scale * 100.0
        );
    }

    fn bump_note_pad(&mut self, delta: f32) {
        self.persist.note_h_pad = (self.persist.note_h_pad + delta).clamp(0.0, 480.0);
        self.persist.save();
        self.status = format!("note padding {:.0}px", self.persist.note_h_pad);
    }

    fn open_note(&mut self, rel: &str, push_nav: bool) {
        self.outline_collapsed.clear();
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        // Create if missing
        if vault.index.get_by_rel(rel).is_none() {
            if let Err(e) = vault.create_note(rel) {
                self.status = format!("create failed: {e}");
                return;
            }
        }
        let content = vault
            .index
            .get_by_rel(rel)
            .map(|n| n.content.clone())
            .or_else(|| vault.read_note_content(rel).ok())
            .unwrap_or_default();
        // Resolve :shortcode: → Unicode so the editor paints real emoji glyphs.
        let content = crate::emoji_util::expand_shortcodes(&content);

        if push_nav {
            if let Some(cur) = self.tabs.get(self.active_tab) {
                self.nav_back.push(cur.rel.clone());
                self.nav_forward.clear();
            }
        }

        if let Some(i) = self.tabs.iter().position(|t| t.rel == rel) {
            self.former_tab = self.active_tab;
            self.active_tab = i;
            return;
        }
        self.former_tab = self.active_tab;
        self.tabs.push(OpenTab {
            rel: rel.to_string(),
            buffer: content,
            dirty: false,
            preview_mode: false,
            scroll_to_line: None,
            scroll_to_heading: None,
            editor_text_id: egui::Id::new(("celestine-editor", rel.to_string())),
        });
        self.active_tab = self.tabs.len() - 1;
        self.show_graph = false;
        self.focus = FocusPane::Editor;
    }

    fn save_active(&mut self) {
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        match vault.write_note(&tab.rel, &tab.buffer) {
            Ok(()) => {
                tab.dirty = false;
                self.status = format!("saved {}", tab.rel);
            }
            Err(e) => self.status = format!("save error: {e}"),
        }
    }

    fn close_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
        if self.tabs.is_empty() {
            self.active_tab = 0;
        }
    }

    fn active_note_id(&self) -> Option<NoteId> {
        let tab = self.tabs.get(self.active_tab)?;
        let vault = self.vault.as_ref()?;
        vault.index.get_by_rel(&tab.rel).map(|n| n.id)
    }

    fn handle_command(&mut self, cmd: Command, ctx: &egui::Context) {
        match cmd {
            Command::QuickOpen => {
                if self.vault.is_some() {
                    self.open_overlay(Overlay::QuickOpen);
                } else {
                    self.open_overlay(Overlay::RecentVaults);
                }
            }
            Command::CommandPalette => self.open_overlay(Overlay::CommandPalette),
            Command::VaultSearch => {
                if self.vault.is_some() {
                    self.open_overlay(Overlay::VaultSearch);
                }
            }
            Command::Shortcuts => self.open_overlay(Overlay::Shortcuts),
            Command::Settings => self.open_overlay(Overlay::Settings),
            Command::Find => self.open_overlay(Overlay::Find),
            Command::ThemePicker => self.open_overlay(Overlay::ThemePicker),
            Command::OpenVault => self.pick_vault_dialog(ctx),
            Command::RecentVaults => self.open_overlay(Overlay::RecentVaults),
            Command::CloseVault => self.close_vault(),
            Command::FontBigger => self.bump_editor_font(1.5),
            Command::FontSmaller => self.bump_editor_font(-1.5),
            Command::FontReset => {
                self.persist.editor_font_size = 15.4;
                self.persist.save();
                self.status = "editor font reset".into();
            }
            Command::Graph => {
                if self.vault.is_some() {
                    self.show_graph = !self.show_graph;
                    if self.show_graph {
                        self.graph.restart_layout();
                    }
                    self.focus = FocusPane::Graph;
                }
            }
            Command::CloseTab => self.close_active_tab(),
            Command::NewNote => {
                if self.vault.is_some() {
                    let name = format!("Untitled {}.md", chrono_lite_stamp());
                    self.open_note(&name, true);
                }
            }
            Command::Save => self.save_active(),
            Command::TogglePreview => {
                if let Some(t) = self.tabs.get_mut(self.active_tab) {
                    t.preview_mode = !t.preview_mode;
                }
            }
            Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::Escape => {
                if self.overlay != Overlay::None {
                    self.overlay = Overlay::None;
                } else if self.show_graph {
                    self.show_graph = false;
                }
            }
            Command::NextTab => {
                if !self.tabs.is_empty() {
                    self.former_tab = self.active_tab;
                    self.active_tab = (self.active_tab + 1) % self.tabs.len();
                }
            }
            Command::PrevTab => {
                if !self.tabs.is_empty() {
                    self.former_tab = self.active_tab;
                    self.active_tab =
                        (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
                }
            }
            Command::FormerTab => {
                let f = self.former_tab.min(self.tabs.len().saturating_sub(1));
                self.former_tab = self.active_tab;
                self.active_tab = f;
            }
            Command::FocusFiles => self.focus = FocusPane::Files,
            Command::FocusOutline => self.focus = FocusPane::Outline,
            Command::NextPane => self.cycle_pane(1),
            Command::PrevPane => self.cycle_pane(-1),
            Command::Tab(n) => {
                let i = (n as usize).saturating_sub(1);
                if i < self.tabs.len() {
                    self.former_tab = self.active_tab;
                    self.active_tab = i;
                }
            }
        }
    }

    fn cycle_pane(&mut self, dir: i32) {
        let panes = [
            FocusPane::Files,
            FocusPane::Editor,
            FocusPane::Outline,
            FocusPane::Graph,
        ];
        let cur = panes.iter().position(|p| *p == self.focus).unwrap_or(1) as i32;
        let next = (cur + dir).rem_euclid(panes.len() as i32) as usize;
        self.focus = panes[next];
        if self.focus == FocusPane::Graph {
            self.show_graph = true;
        }
    }

    fn open_overlay(&mut self, o: Overlay) {
        self.overlay = o;
        self.overlay_query.clear();
        self.overlay_selected = 0;
        self.overlay_anim = 0.0;
        self.editor_focused = false;
        self.overlay_focus_pending = true;
    }

    fn pick_vault_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_vault(path, ctx);
        }
    }
}

fn chrono_lite_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 100_000)
        .unwrap_or(0);
    t.to_string()
}

impl eframe::App for CelestineApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        // Overlay fade-in
        if self.overlay != Overlay::None {
            self.overlay_anim = (self.overlay_anim + dt * 8.0).min(1.0);
        } else {
            self.overlay_anim = 0.0;
        }

        if let Some(vault) = self.vault.as_mut() {
            if vault.poll_fs() {
                self.status = "index refreshed".into();
            }
        }

        if let Some(cmd) = self.keymap.consume(ctx, self.editor_focused) {
            self.handle_command(cmd, ctx);
        }

        let palette = theme_apply::ThemePalette::from_tokens(&self.theme);
        let accent = palette.accent;
        let muted = palette.muted;
        let text_c = palette.text;

        // Top bar — flea chrome: path/brand left, naked icons right
        egui::Panel::top("top")
            .exact_size(36.0)
            .frame(theme_apply::panel_frame(&ctx))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("celestine").strong().color(accent));
                    if let Some(v) = &self.vault {
                        ui.label(
                            RichText::new(format!("  {}", v.root.display()))
                                .color(muted)
                                .size(12.5),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if naked_icon(ui, "sliders", false, accent, muted)
                            .on_hover_text("Settings")
                            .clicked()
                        {
                            self.open_overlay(Overlay::Settings);
                        }
                        if naked_icon(
                            ui,
                            "panel-right",
                            self.persist.show_right_sidebar,
                            accent,
                            muted,
                        )
                        .on_hover_text("Toggle outline")
                        .clicked()
                        {
                            self.toggle_right_sidebar();
                        }
                        if naked_icon(
                            ui,
                            "panel-left",
                            self.persist.show_left_sidebar,
                            accent,
                            muted,
                        )
                        .on_hover_text("Toggle files")
                        .clicked()
                        {
                            self.toggle_left_sidebar();
                        }
                        if naked_icon(ui, "search", false, accent, muted)
                            .on_hover_text(format!(
                                "Quick open {}",
                                self.keymap.display_chord(Command::QuickOpen)
                            ))
                            .clicked()
                        {
                            self.open_overlay(Overlay::QuickOpen);
                        }
                        if naked_button(
                            ui,
                            RichText::new(self.theme_name.as_str()).size(12.0).color(muted),
                        )
                        .on_hover_text("Themes")
                        .clicked()
                        {
                            self.open_overlay(Overlay::ThemePicker);
                        }
                        if naked_button(
                            ui,
                            RichText::new("vault").size(12.0).color(muted),
                        )
                        .on_hover_text("Open vault")
                        .clicked()
                        {
                            self.pick_vault_dialog(ctx);
                        }
                    });
                });
            });

        // Tabs strip
        if !self.tabs.is_empty() {
            egui::Panel::top("tabs")
                .exact_size(30.0)
                .frame(theme_apply::panel_frame(&ctx))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        let mut switch_to = None;
                        let mut close_i = None;
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let dirty = if tab.dirty { "·" } else { "" };
                            let name = Path::new(&tab.rel)
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_else(|| tab.rel.clone());
                            let selected = i == self.active_tab;
                            let label = RichText::new(format!("{name}{dirty}"))
                                .size(12.5)
                                .color(if selected { text_c } else { muted });
                            let resp = naked_button(ui, label);
                            if selected {
                                let r = resp.rect;
                                ui.painter().hline(
                                    r.left()..=r.right(),
                                    r.bottom() - 1.0,
                                    Stroke::new(1.5_f32, accent),
                                );
                            }
                            if resp.clicked() {
                                switch_to = Some(i);
                            }
                            if naked_button(ui, RichText::new("×").size(12.0).color(muted))
                                .on_hover_text(self.keymap.display_chord(Command::CloseTab))
                                .clicked()
                            {
                                close_i = Some(i);
                            }
                            ui.add_space(6.0);
                        }
                        if let Some(i) = switch_to {
                            self.former_tab = self.active_tab;
                            self.active_tab = i;
                        }
                        if let Some(i) = close_i {
                            self.tabs.remove(i);
                            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                                self.active_tab = self.tabs.len() - 1;
                            }
                        }
                    });
                });
        }

        // Footer cheat strip
        egui::Panel::bottom("foot").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).monospace().color(muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let strip = match self.focus {
                        FocusPane::Files => format!(
                            "files {} · open {} · ?",
                            self.keymap.display_chord(Command::FocusFiles),
                            self.keymap.display_chord(Command::QuickOpen)
                        ),
                        FocusPane::Editor => format!(
                            "save {} · preview {} · close {} · ?",
                            self.keymap.display_chord(Command::Save),
                            self.keymap.display_chord(Command::TogglePreview),
                            self.keymap.display_chord(Command::CloseTab)
                        ),
                        FocusPane::Outline => "outline · click heading".into(),
                        FocusPane::Graph => format!(
                            "graph {} · drag pan · scroll zoom",
                            self.keymap.display_chord(Command::Graph)
                        ),
                    };
                    ui.label(RichText::new(strip).monospace().color(accent));
                });
            });
        });

        // Left: files (collapsible like Obsidian)
        if self.persist.show_left_sidebar {
            egui::Panel::left("files")
                .default_size(220.0)
                .frame(theme_apply::panel_frame(&ctx))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("FILES")
                                .size(11.0)
                                .color(muted)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if naked_icon(ui, "chevron-left", false, accent, muted)
                                .on_hover_text("Hide files")
                                .clicked()
                            {
                                self.toggle_left_sidebar();
                            }
                        });
                    });
                    ui.label(
                        RichText::new("right-click folder → mark")
                            .size(10.0)
                            .color(muted.gamma_multiply(0.75)),
                    );
                    ui.add_space(4.0);
                    if self.vault.is_some() {
                        let mut paths = self
                            .vault
                            .as_ref()
                            .map(|v| v.list_md_rel_paths())
                            .unwrap_or_default();
                        paths.sort();
                        let tree = file_tree::build_tree(&paths);
                        let active = self.tabs.get(self.active_tab).map(|t| t.rel.as_str());
                        let outcome = ScrollArea::vertical()
                            .show(ui, |ui| {
                                file_tree::show_tree(
                                    ui,
                                    &tree,
                                    &mut self.tree_expanded,
                                    &mut self.folder_icons,
                                    &palette,
                                    muted,
                                    active,
                                )
                            })
                            .inner;
                        if outcome.icons_changed {
                            self.persist_folder_icons();
                            self.status = "folder icon saved".into();
                        }
                        if let Some(rel) = outcome.open_rel {
                            if let Some(parent) = Path::new(&rel).parent() {
                                let mut acc = String::new();
                                for part in parent.components() {
                                    let p = part.as_os_str().to_string_lossy();
                                    if p == "." || p.is_empty() {
                                        continue;
                                    }
                                    if !acc.is_empty() {
                                        acc.push('/');
                                    }
                                    acc.push_str(&p);
                                    self.tree_expanded.insert(acc.clone());
                                }
                            }
                            self.open_note(&rel, true);
                        }
                    } else {
                        ui.label(RichText::new("(no vault)").color(muted));
                        if naked_button(ui, RichText::new("open folder…").color(accent)).clicked()
                        {
                            self.pick_vault_dialog(ctx);
                        }
                    }
                });
        }

        // Right: outline + backlinks
        if self.persist.show_right_sidebar {
            egui::Panel::right("meta")
                .default_size(220.0)
                .min_size(160.0)
                .max_size(360.0)
                .frame(theme_apply::panel_frame(&ctx))
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("OUTLINE")
                                .size(11.0)
                                .color(muted)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if naked_icon(ui, "chevron-right", false, accent, muted)
                                .on_hover_text("Hide outline")
                                .clicked()
                            {
                                self.toggle_right_sidebar();
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let mut jump: Option<(usize, String)> = None;
                    let mut open_backlink: Option<String> = None;
                    if let (Some(vault), Some(tab)) =
                        (self.vault.as_ref(), self.tabs.get(self.active_tab))
                    {
                        let parsed = extract_note_data(&tab.buffer);
                        let tree = outline_tree::build(&parsed.headings);
                        ScrollArea::vertical()
                            .max_height(ui.available_height() * 0.55)
                            .show(ui, |ui| {
                                ui.set_max_width(ui.available_width());
                                if tree.is_empty() {
                                    ui.label(RichText::new("(no headings)").color(muted));
                                } else {
                                    jump = outline_tree::show(
                                        ui,
                                        &tree,
                                        &mut self.outline_collapsed,
                                        &palette.heading,
                                        muted,
                                        text_c,
                                    );
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("BACKLINKS").size(11.0).strong().color(muted),
                        );
                        if let Some(meta) = vault.index.get_by_rel(&tab.rel) {
                            let backs = vault.graph.backlinks(&vault.index, meta.id);
                            if backs.is_empty() {
                                ui.label(RichText::new("(none)").color(muted));
                            } else {
                                for id in backs {
                                    if let Some(n) = vault.index.get(id) {
                                        let w = ui.available_width().max(40.0);
                                        ui.set_max_width(w);
                                        let label = RichText::new(&n.rel).color(text_c);
                                        let resp = ui.add(
                                            egui::Label::new(label)
                                                .wrap()
                                                .sense(egui::Sense::click()),
                                        );
                                        if resp.clicked() {
                                            open_backlink = Some(n.rel.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some((line, heading)) = jump {
                        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                            tab.scroll_to_line = Some(line);
                            tab.scroll_to_heading = Some(heading);
                        }
                    }
                    if let Some(rel) = open_backlink {
                        self.open_note(&rel, true);
                    }
                });
        }

        // Center
        egui::CentralPanel::default()
            .frame(theme_apply::content_frame(&self.theme))
            .show(ui, |ui| {
            if self.show_graph {
                ui.label(
                    RichText::new(format!(
                        "── graph {} ──",
                        self.keymap.display_chord(Command::Graph)
                    ))
                    .monospace()
                    .color(accent),
                );
                if let Some(vault) = &self.vault {
                    let center = self.active_note_id();
                    if let Some(rel) =
                        self.graph
                            .show(ui, vault, center, accent, muted, text_c, &palette)
                    {
                        self.open_note(&rel, true);
                        self.show_graph = false;
                    }
                }
            } else if self.vault.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.label(RichText::new("celestine").monospace().strong().size(28.0).color(accent));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("no vault open")
                            .monospace()
                            .color(muted),
                    );
                    ui.add_space(16.0);
                    if naked_button(
                        ui,
                        RichText::new(format!(
                            "open vault…  {}",
                            self.keymap.display_chord(Command::OpenVault)
                        ))
                        .color(accent),
                    )
                    .clicked()
                    {
                        self.pick_vault_dialog(ctx);
                    }
                    if naked_button(
                        ui,
                        RichText::new(format!(
                            "recent vaults  {}",
                            self.keymap.display_chord(Command::RecentVaults)
                        ))
                        .color(muted),
                    )
                    .clicked()
                    {
                        self.open_overlay(Overlay::RecentVaults);
                    }
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(format!(
                            "themes {} · font +/- {} {}",
                            self.keymap.display_chord(Command::ThemePicker),
                            self.keymap.display_chord(Command::FontBigger),
                            self.keymap.display_chord(Command::FontSmaller),
                        ))
                        .monospace()
                        .color(muted),
                    );
                });
            } else if let Some(tab_idx) = self.tabs.get(self.active_tab).map(|_| self.active_tab) {
                // While an overlay is open, keep the note visible but disable the editor
                // so it cannot steal keyboard focus from the submenu.
                if self.overlay != Overlay::None {
                    ui.disable();
                }
                let preview_mode = self.tabs[tab_idx].preview_mode;
                let rel = self.tabs[tab_idx].rel.clone();
                let md_font = self.persist.editor_font_size;
                let stats = text_stats(&self.tabs[tab_idx].buffer);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("── {rel} ──")).monospace().color(accent));
                    let mode = if preview_mode { "PREVIEW" } else { "EDIT" };
                    ui.label(
                        RichText::new(format!(
                            "{mode} {} · font {:.0}px ({} {})",
                            self.keymap.display_chord(Command::TogglePreview),
                            md_font,
                            self.keymap.display_chord(Command::FontBigger),
                            self.keymap.display_chord(Command::FontSmaller),
                        ))
                        .monospace()
                        .color(muted),
                    );
                });

                // Pin stats to the bottom of the note pane; editor/preview fills the rest.
                let stats_h = 24.0_f32;
                egui::Panel::bottom("note-stats")
                    .exact_size(stats_h)
                    .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2)))
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} lines · {} words · {} symbols",
                                    stats.lines, stats.words, stats.symbols
                                ))
                                .monospace()
                                .color(muted),
                            );
                        });
                    });

                // Full-width note pane: line gutter on the far left, solid scrollbar on
                // the far right; L/R padding shrinks with the window so text still fits.
                let fill_h = ui.available_height().max(80.0);
                let fill_w = ui.available_width().max(1.0);
                let preferred_pad = self.persist.note_h_pad;
                const MIN_TEXT_W: f32 = 200.0_f32;

                if preview_mode {
                    let body = {
                        let buf = &self.tabs[tab_idx].buffer;
                        let parsed = extract_note_data(buf);
                        parsed.body
                    };
                    let scroll_heading = self.tabs[tab_idx].scroll_to_heading.clone();
                    let outcome = if let Some(vault) = &self.vault {
                        ScrollArea::vertical()
                            .id_salt("preview-scroll")
                            .auto_shrink([false, false])
                            .max_width(fill_w)
                            .min_scrolled_height(fill_h)
                            .show(ui, |ui| {
                                ui.set_min_height(fill_h);
                                let inner_w = ui.available_width().max(1.0);
                                let (pad, text_w) =
                                    note_side_pad(inner_w, 0.0, preferred_pad, MIN_TEXT_W);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    ui.add_space(pad);
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(text_w, fill_h),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_max_width(text_w);
                                            ui.set_width(text_w);
                                            preview::show_preview(
                                                ui,
                                                &body,
                                                &rel,
                                                &vault.index,
                                                accent,
                                                muted,
                                                text_c,
                                                md_font,
                                                scroll_heading.as_deref(),
                                                &palette.heading,
                                            )
                                        },
                                    )
                                    .inner
                                })
                                .inner
                            })
                            .inner
                    } else {
                        preview::PreviewOutcome {
                            open_rel: None,
                            heading_rects: vec![],
                        }
                    };
                    if scroll_heading.is_some() {
                        if let Some(tab) = self.tabs.get_mut(tab_idx) {
                            tab.scroll_to_heading = None;
                            tab.scroll_to_line = None;
                        }
                    }
                    if let Some(r) = outcome.open_rel {
                        if self
                            .vault
                            .as_ref()
                            .and_then(|v| v.index.get_by_rel(&r))
                            .is_none()
                        {
                            self.status = format!("creating {r}");
                        }
                        self.open_note(&r, true);
                    }
                } else {
                    let font = FontId::monospace(md_font);
                    let text_id = self.tabs[tab_idx].editor_text_id;
                    let jump_line = self.tabs[tab_idx].scroll_to_line.take();
                    let jump_heading = self.tabs[tab_idx].scroll_to_heading.take();
                    let jump_char = jump_heading
                        .as_deref()
                        .and_then(|h| {
                            char_offset_for_heading(&self.tabs[tab_idx].buffer, h)
                        })
                        .or_else(|| {
                            jump_line.map(|line| {
                                char_offset_at_line(&self.tabs[tab_idx].buffer, line)
                            })
                        });
                    let gutter_w = line_gutter_width(stats.lines, md_font);
                    let gutter_gap = 6.0_f32;
                    let row_h = (md_font * 1.35).max(1.0);
                    let desired_rows =
                        ((fill_h / row_h).ceil() as usize).clamp(24, 200);
                    let (changed, focused) = ScrollArea::vertical()
                        .id_salt("editor-scroll")
                        .auto_shrink([false, false])
                        .max_width(fill_w)
                        .min_scrolled_height(fill_h)
                        .show(ui, |ui| {
                            ui.set_min_height(fill_h);
                            let inner_w = ui.available_width().max(1.0);
                            let (pad, text_w) = note_side_pad(
                                inner_w,
                                gutter_w + gutter_gap,
                                preferred_pad,
                                MIN_TEXT_W,
                            );
                            ui.horizontal(|ui| {
                                // Manual gaps only — avoid item_spacing blowing past inner_w.
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let headings = palette.heading;
                                let mut layouter =
                                    |ui: &egui::Ui,
                                     text: &dyn egui::TextBuffer,
                                     wrap_width: f32| {
                                        let text = text.as_str();
                                        let w = if wrap_width.is_finite()
                                            && wrap_width > 1.0
                                        {
                                            wrap_width.min(text_w)
                                        } else {
                                            text_w
                                        };
                                        md_edit::layout(
                                            ui,
                                            text,
                                            w,
                                            md_font,
                                            text_c,
                                            muted,
                                            accent,
                                            &headings,
                                        )
                                    };

                                // Far-left line gutter (scrolls with content).
                                let gutter_placeholder = ui
                                    .allocate_exact_size(
                                        egui::vec2(gutter_w, fill_h),
                                        egui::Sense::hover(),
                                    )
                                    .0;
                                ui.add_space(gutter_gap + pad);

                                let te = TextEdit::multiline(
                                    &mut self.tabs[tab_idx].buffer,
                                )
                                .id(text_id)
                                .font(font)
                                .frame(egui::Frame::NONE)
                                .desired_width(text_w)
                                .desired_rows(desired_rows)
                                .layouter(&mut layouter);
                                let mut out = te.show(ui);

                                ui.add_space(pad);

                                let need_h = fill_h.max(out.galley.size().y);
                                if let Some(char_pos) = jump_char {
                                    let id = out.response.id;
                                    out.response.request_focus();
                                    out.state.cursor.set_char_range(Some(
                                        CCursorRange::one(CCursor::new(char_pos)),
                                    ));
                                    out.state.store(ui.ctx(), id);
                                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                                }

                                let gutter_rect = egui::Rect::from_min_size(
                                    gutter_placeholder.min,
                                    egui::vec2(gutter_w, need_h),
                                );
                                paint_line_numbers(
                                    ui,
                                    gutter_rect,
                                    &out.galley,
                                    out.galley_pos,
                                    md_font,
                                    muted,
                                );

                                (out.response.changed(), out.response.has_focus())
                            })
                            .inner
                        })
                        .inner;
                    self.editor_focused = focused;
                    if changed {
                        let expanded =
                            crate::emoji_util::expand_shortcodes(&self.tabs[tab_idx].buffer);
                        if expanded != self.tabs[tab_idx].buffer {
                            self.tabs[tab_idx].buffer = expanded;
                        }
                        self.tabs[tab_idx].dirty = true;
                    }
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "no note open\n{} quick open · {} new note",
                            self.keymap.display_chord(Command::QuickOpen),
                            self.keymap.display_chord(Command::NewNote)
                        ))
                        .monospace()
                        .color(muted),
                    );
                });
            }
        });

        self.draw_overlay(ctx, accent, muted);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl CelestineApp {
    fn draw_overlay(&mut self, ctx: &egui::Context, accent: egui::Color32, muted: egui::Color32) {
        if self.overlay == Overlay::None {
            return;
        }
        let alpha = self.overlay_anim;
        let title = match self.overlay {
            Overlay::QuickOpen => format!("quick open {}", self.keymap.display_chord(Command::QuickOpen)),
            Overlay::CommandPalette => {
                format!("commands {}", self.keymap.display_chord(Command::CommandPalette))
            }
            Overlay::VaultSearch => {
                format!("search {}", self.keymap.display_chord(Command::VaultSearch))
            }
            Overlay::Shortcuts => "shortcuts  ?".into(),
            Overlay::Settings => "settings".into(),
            Overlay::Find => format!("find {}", self.keymap.display_chord(Command::Find)),
            Overlay::ThemePicker => {
                format!("themes {}", self.keymap.display_chord(Command::ThemePicker))
            }
            Overlay::RecentVaults => {
                format!("recent {}", self.keymap.display_chord(Command::RecentVaults))
            }
            Overlay::None => return,
        };

        // Dim stays in Middle; the modal is Foreground so it can never be covered
        // when the full-screen dim is clicked (same-Order areas reorder on interact).
        let dim_a = ((140.0 * alpha.clamp(0.0, 1.0)) as u8).max(1);
        egui::Area::new(egui::Id::new("overlay-dim"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Middle)
            .interactable(true)
            .sense(egui::Sense::click())
            .show(ctx, |ui| {
                let screen = ctx.content_rect();
                let resp = ui.allocate_response(screen.size(), egui::Sense::click());
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    egui::Color32::from_black_alpha(dim_a),
                );
                if resp.clicked() {
                    self.overlay = Overlay::None;
                }
            });

        egui::Area::new(egui::Id::new("overlay"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 64.0 * alpha))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                theme_apply::overlay_frame(&self.theme).show(ui, |ui| {
                    ui.set_min_width(500.0);
                    ui.set_max_width(640.0);
                    ui.label(
                        RichText::new(title)
                            .strong()
                            .size(15.0)
                            .color(accent),
                    );
                    ui.add_space(6.0);
                    match self.overlay {
                        Overlay::Shortcuts => self.ui_shortcuts(ui, muted),
                        Overlay::Settings => self.ui_settings(ui, ctx, accent, muted),
                        Overlay::ThemePicker => self.ui_theme_picker(ui, ctx, accent, muted),
                        Overlay::RecentVaults => self.ui_recent_vaults(ui, ctx, accent, muted),
                        Overlay::Find => self.ui_find(ui),
                        Overlay::QuickOpen | Overlay::CommandPalette | Overlay::VaultSearch => {
                            self.ui_fuzzy(ui, accent, muted);
                        }
                        Overlay::None => {}
                    }
                });
            });
    }

    fn ui_shortcuts(&self, ui: &mut Ui, muted: egui::Color32) {
        ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (cmd, chord) in self.keymap.all_listed() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{chord:16}")).monospace().color(muted));
                    let label = if let Command::Tab(n) = cmd {
                        format!("Jump tab {n}")
                    } else {
                        cmd.label().to_string()
                    };
                    ui.label(RichText::new(label).monospace());
                });
            }
        });
        ui.label(RichText::new("esc dismiss").monospace().color(muted));
    }

    fn ui_settings(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        accent: egui::Color32,
        muted: egui::Color32,
    ) {
        if self.overlay == Overlay::Settings {
            if naked_button(ui, RichText::new("open vault folder…").color(accent)).clicked() {
                self.pick_vault_dialog(ctx);
                self.overlay = Overlay::None;
            }
            if naked_button(ui, RichText::new("recent vaults…").color(muted)).clicked() {
                self.open_overlay(Overlay::RecentVaults);
                return;
            }
            if self.vault.is_some()
                && naked_button(ui, RichText::new("close vault").color(muted)).clicked()
            {
                self.close_vault();
                self.overlay = Overlay::None;
            }
            ui.add_space(10.0);
            ui.label(RichText::new("UI FONT").size(11.0).strong().color(muted));
            ui.horizontal(|ui| {
                if naked_button(ui, RichText::new("−").color(accent)).clicked() {
                    self.bump_ui_font(-0.05, ctx);
                }
                ui.label(RichText::new(format!(
                    "{:.0}%",
                    self.persist.ui_font_scale * 100.0
                )));
                if naked_button(ui, RichText::new("+").color(accent)).clicked() {
                    self.bump_ui_font(0.05, ctx);
                }
                if naked_button(ui, RichText::new("reset").color(muted)).clicked() {
                    self.persist.ui_font_scale = 1.10;
                    self.persist.save();
                    theme_apply::apply_theme(ctx, &self.theme, self.persist.ui_font_scale);
                    self.status = "UI font reset".into();
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new("EDITOR FONT").size(11.0).strong().color(muted));
            ui.horizontal(|ui| {
                if naked_button(ui, RichText::new("−").color(accent)).clicked() {
                    self.bump_editor_font(-1.5);
                }
                ui.label(RichText::new(format!("{:.0}px", self.persist.editor_font_size)));
                if naked_button(ui, RichText::new("+").color(accent)).clicked() {
                    self.bump_editor_font(1.5);
                }
                if naked_button(ui, RichText::new("reset").color(muted)).clicked() {
                    self.handle_command(Command::FontReset, ctx);
                }
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new("NOTE PADDING (L/R)")
                    .size(11.0)
                    .strong()
                    .color(muted),
            );
            ui.horizontal(|ui| {
                if naked_button(ui, RichText::new("−").color(accent)).clicked() {
                    self.bump_note_pad(-16.0);
                }
                ui.label(RichText::new(format!("{:.0}px", self.persist.note_h_pad)));
                if naked_button(ui, RichText::new("+").color(accent)).clicked() {
                    self.bump_note_pad(16.0);
                }
                if naked_button(ui, RichText::new("reset").color(muted)).clicked() {
                    self.persist.note_h_pad = 120.0;
                    self.persist.save();
                    self.status = "note padding reset".into();
                }
            });
            ui.label(
                RichText::new(
                    "preferred L/R pad — shrinks automatically when the window is narrow",
                )
                .size(10.0)
                .color(muted.gamma_multiply(0.75)),
            );
            ui.add_space(10.0);
            ui.label(RichText::new("SIDEBARS").size(11.0).strong().color(muted));
            ui.horizontal(|ui| {
                if sidebar_toggle(
                    ui,
                    "files",
                    self.persist.show_left_sidebar,
                    accent,
                    muted,
                )
                .clicked()
                {
                    self.toggle_left_sidebar();
                }
                ui.add_space(8.0);
                if sidebar_toggle(
                    ui,
                    "outline",
                    self.persist.show_right_sidebar,
                    accent,
                    muted,
                )
                .clicked()
                {
                    self.toggle_right_sidebar();
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new("THEME").size(11.0).strong().color(muted));
            if naked_button(ui, RichText::new("choose theme…").color(accent)).clicked() {
                self.open_overlay(Overlay::ThemePicker);
                return;
            }
            ui.label(
                RichText::new(format!("current: {}", self.theme_name)).color(muted),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("keymap profile: {}", self.keymap.profile)),
            );
            ui.label(
                RichText::new("edit ~/.config/celestine/keybindings.toml to remap").color(muted),
            );
        }
    }

    fn ui_theme_picker(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        accent: egui::Color32,
        muted: egui::Color32,
    ) {
        let filter_id = egui::Id::new("celestine-theme-filter");
        let resp = ui.add(
            TextEdit::singleline(&mut self.overlay_query)
                .id(filter_id)
                .hint_text("filter themes — ↑↓ enter")
                .desired_width(f32::INFINITY),
        );
        if self.overlay_focus_pending {
            resp.request_focus();
            self.overlay_focus_pending = false;
            // Start selection on the active theme
            let themes = list_available_themes();
            if let Some(i) = themes.iter().position(|t| t == &self.theme_name) {
                self.overlay_selected = i;
            }
        }

        let q = self.overlay_query.to_lowercase();
        let themes: Vec<String> = list_available_themes()
            .into_iter()
            .filter(|t| q.is_empty() || t.to_lowercase().contains(&q))
            .collect();

        if self.overlay_selected >= themes.len() && !themes.is_empty() {
            self.overlay_selected = themes.len() - 1;
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.overlay_selected =
                (self.overlay_selected + 1).min(themes.len().saturating_sub(1));
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.overlay_selected = self.overlay_selected.saturating_sub(1);
        }

        let text_c = color(self.theme.text_normal);
        let mut pick = None;
        ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (i, name) in themes.iter().enumerate() {
                let selected = i == self.overlay_selected;
                let is_current = name == &self.theme_name;
                let label = if is_current {
                    format!("★  {name}")
                } else {
                    format!("   {name}")
                };
                let row_color = if selected {
                    accent
                } else if is_current {
                    text_c
                } else {
                    muted
                };
                let row = ui.add(
                    egui::Button::new(RichText::new(label).color(row_color))
                        .frame(false)
                        .selected(selected),
                );
                if selected {
                    row.scroll_to_me(Some(egui::Align::Center));
                }
                if row.clicked() {
                    pick = Some(name.clone());
                }
            }
        });

        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(name) = themes.get(self.overlay_selected) {
                pick = Some(name.clone());
            }
        }

        ui.add_space(4.0);
        ui.label(
            RichText::new("↑↓ move · enter apply · esc dismiss")
                .size(11.0)
                .color(muted),
        );

        if let Some(name) = pick {
            self.set_theme(&name, ctx);
            self.overlay = Overlay::None;
        }
    }

    fn ui_recent_vaults(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        accent: egui::Color32,
        muted: egui::Color32,
    ) {
        self.persist.clear_recent_missing();
        if naked_button(
            ui,
            RichText::new(format!(
                "browse for vault…  {}",
                self.keymap.display_chord(Command::OpenVault)
            ))
            .color(accent),
        )
        .clicked()
        {
            self.pick_vault_dialog(ctx);
            self.overlay = Overlay::None;
            return;
        }
        ui.add_space(8.0);
        if self.persist.recent_vaults.is_empty() {
            ui.label(RichText::new("(no recent vaults)").color(muted));
            return;
        }
        let mut open = None;
        ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for path in &self.persist.recent_vaults {
                let is_last = self.persist.last_vault.as_deref() == Some(path.as_str());
                let label = if is_last {
                    format!("★  {path}")
                } else {
                    format!("   {path}")
                };
                if naked_button(
                    ui,
                    RichText::new(label).color(if is_last { accent } else { muted }),
                )
                .clicked()
                {
                    open = Some(path.clone());
                }
            }
        });
        if let Some(path) = open {
            self.overlay = Overlay::None;
            self.open_vault(PathBuf::from(path), ctx);
        }
    }

    fn ui_find(&mut self, ui: &mut Ui) {
        let resp = ui.add(
            TextEdit::singleline(&mut self.find_query)
                .id(egui::Id::new("celestine-find"))
                .hint_text("find in note")
                .desired_width(f32::INFINITY),
        );
        if self.overlay_focus_pending {
            resp.request_focus();
            self.overlay_focus_pending = false;
        }
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if !self.find_query.is_empty() {
                let count = tab.buffer.matches(&self.find_query).count();
                ui.label(RichText::new(format!("{count} matches")).monospace());
            }
        }
    }

    fn ui_fuzzy(&mut self, ui: &mut Ui, accent: egui::Color32, muted: egui::Color32) {
        let resp = ui.add(
            TextEdit::singleline(&mut self.overlay_query)
                .id(egui::Id::new("celestine-fuzzy-filter"))
                .hint_text("type to filter")
                .desired_width(f32::INFINITY),
        );
        if self.overlay_focus_pending {
            resp.request_focus();
            self.overlay_focus_pending = false;
        }

        let matcher = SkimMatcherV2::default();
        let mut items: Vec<(i64, String, String)> = Vec::new();

        match self.overlay {
            Overlay::QuickOpen | Overlay::VaultSearch => {
                if let Some(vault) = &self.vault {
                    for n in vault.index.notes() {
                        let hay = format!("{} {}", n.rel, n.aliases.join(" "));
                        let score = if self.overlay_query.is_empty() {
                            Some(0)
                        } else {
                            matcher.fuzzy_match(&hay, &self.overlay_query)
                        };
                        if let Some(s) = score {
                            let preview = if self.overlay == Overlay::VaultSearch {
                                n.body.lines().next().unwrap_or("").to_string()
                            } else {
                                n.aliases.first().cloned().unwrap_or_default()
                            };
                            items.push((s, n.rel.clone(), preview));
                        }
                    }
                }
            }
            Overlay::CommandPalette => {
                let cmds = [
                    Command::OpenVault,
                    Command::RecentVaults,
                    Command::CloseVault,
                    Command::QuickOpen,
                    Command::ThemePicker,
                    Command::Graph,
                    Command::NewNote,
                    Command::Save,
                    Command::TogglePreview,
                    Command::VaultSearch,
                    Command::FontBigger,
                    Command::FontSmaller,
                    Command::FontReset,
                    Command::Settings,
                    Command::Shortcuts,
                    Command::CloseTab,
                    Command::FocusFiles,
                    Command::FocusOutline,
                ];
                for c in cmds {
                    let label = format!("{}  {}", c.label(), self.keymap.display_chord(c));
                    let score = if self.overlay_query.is_empty() {
                        Some(0)
                    } else {
                        matcher.fuzzy_match(&label, &self.overlay_query)
                    };
                    if let Some(s) = score {
                        items.push((s, format!("cmd:{}", c.id()), label));
                    }
                }
            }
            _ => {}
        }

        items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        if self.overlay_selected >= items.len() && !items.is_empty() {
            self.overlay_selected = items.len() - 1;
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.overlay_selected = (self.overlay_selected + 1).min(items.len().saturating_sub(1));
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.overlay_selected = self.overlay_selected.saturating_sub(1);
        }

        let mut chosen = None;
        ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for (i, (_s, id, label)) in items.iter().enumerate() {
                let selected = i == self.overlay_selected;
                if ui
                    .selectable_label(
                        selected,
                        RichText::new(format!("{id}  {label}"))
                            .monospace()
                            .color(if selected { accent } else { muted }),
                    )
                    .clicked()
                {
                    chosen = Some(id.clone());
                }
            }
        });

        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some((_s, id, _)) = items.get(self.overlay_selected) {
                chosen = Some(id.clone());
            } else if self.overlay == Overlay::QuickOpen && !self.overlay_query.is_empty() {
                chosen = Some(ensure_md_path(&self.overlay_query));
            }
        }

        if let Some(id) = chosen {
            if let Some(cmd_id) = id.strip_prefix("cmd:") {
                self.overlay = Overlay::None;
                if let Some(cmd) = cmd_from_id(cmd_id) {
                    self.handle_command(cmd, ui.ctx());
                }
            } else {
                self.overlay = Overlay::None;
                self.open_note(&id, true);
            }
        }
    }
}

fn ensure_md_path(q: &str) -> String {
    if q.to_lowercase().ends_with(".md") {
        q.to_string()
    } else {
        format!("{q}.md")
    }
}

fn char_offset_at_line(text: &str, line: usize) -> usize {
    let mut chars = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i == line {
            return chars;
        }
        chars += l.chars().count();
    }
    chars
}

fn char_offset_for_heading(text: &str, heading: &str) -> Option<usize> {
    let heading = heading.trim();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let after_hashes = trimmed.trim_start_matches('#').trim_start();
        if after_hashes == heading {
            return Some(char_offset_at_line(text, i));
        }
    }
    None
}

struct TextStats {
    lines: usize,
    words: usize,
    symbols: usize,
}

fn text_stats(text: &str) -> TextStats {
    let lines = if text.is_empty() {
        1
    } else {
        text.bytes().filter(|&b| b == b'\n').count() + 1
    };
    let words = text.split_whitespace().count();
    let symbols = text.chars().count();
    TextStats {
        lines,
        words,
        symbols,
    }
}

fn line_gutter_width(lines: usize, font_size: f32) -> f32 {
    let digits = ((lines.max(1) as f32).log10().floor() as usize) + 1;
    digits.max(2) as f32 * font_size * 0.62 + 10.0
}

/// Preferred L/R note padding, reduced when the pane is too narrow to keep `min_text`.
/// `chrome` is space already reserved on the left (line gutter + gap).
/// Returns `(pad_each_side, text_width)`.
fn note_side_pad(inner_w: f32, chrome: f32, preferred_pad: f32, min_text: f32) -> (f32, f32) {
    let usable = (inner_w - chrome).max(1.0);
    let max_pad = ((usable - min_text) * 0.5).max(0.0);
    let pad = preferred_pad.clamp(0.0, max_pad);
    let text_w = (usable - 2.0 * pad).max(40.0_f32.min(usable));
    (pad, text_w)
}

fn paint_line_numbers(
    ui: &Ui,
    gutter_rect: egui::Rect,
    galley: &egui::Galley,
    galley_pos: egui::Pos2,
    font_size: f32,
    color: egui::Color32,
) {
    let painter = ui.painter_at(gutter_rect);
    let font = FontId::monospace(font_size * 0.9);
    let mut src_line = 1usize;
    let mut at_line_start = true;
    for row in &galley.rows {
        if at_line_start {
            let y = galley_pos.y + row.rect().min.y;
            painter.text(
                egui::pos2(gutter_rect.right() - 4.0, y),
                egui::Align2::RIGHT_TOP,
                format!("{src_line}"),
                font.clone(),
                color,
            );
        }
        if row.ends_with_newline {
            src_line += 1;
            at_line_start = true;
        } else {
            at_line_start = false;
        }
    }
}

fn cmd_from_id(id: &str) -> Option<Command> {
    Some(match id {
        "quick_open" => Command::QuickOpen,
        "graph" => Command::Graph,
        "new_note" => Command::NewNote,
        "save" => Command::Save,
        "toggle_preview" => Command::TogglePreview,
        "vault_search" => Command::VaultSearch,
        "settings" => Command::Settings,
        "shortcuts" => Command::Shortcuts,
        "close_tab" => Command::CloseTab,
        "focus_files" => Command::FocusFiles,
        "focus_outline" => Command::FocusOutline,
        "command_palette" => Command::CommandPalette,
        "theme_picker" => Command::ThemePicker,
        "open_vault" => Command::OpenVault,
        "recent_vaults" => Command::RecentVaults,
        "close_vault" => Command::CloseVault,
        "font_bigger" => Command::FontBigger,
        "font_smaller" => Command::FontSmaller,
        "font_reset" => Command::FontReset,
        _ => return None,
    })
}
