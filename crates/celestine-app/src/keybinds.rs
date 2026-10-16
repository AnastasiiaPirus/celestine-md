use egui::{Key, KeyboardShortcut, Modifiers};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    QuickOpen,
    CommandPalette,
    Graph,
    CloseTab,
    NewNote,
    Save,
    TogglePreview,
    Find,
    VaultSearch,
    Settings,
    Quit,
    Shortcuts,
    ThemePicker,
    OpenVault,
    RecentVaults,
    CloseVault,
    FontBigger,
    FontSmaller,
    FontReset,
    NextTab,
    PrevTab,
    FormerTab,
    FocusFiles,
    FocusOutline,
    NextPane,
    PrevPane,
    Tab(u8),
    Escape,
    Rename,
    NewFolder,
    DeleteItem,
    ToggleLeftSidebar,
    ToggleRightSidebar,
    NavBack,
    NavForward,
    RevealInTree,
}

impl Command {
    pub fn label(self) -> &'static str {
        match self {
            Command::QuickOpen => "Quick open",
            Command::CommandPalette => "Command palette",
            Command::Graph => "Graph view",
            Command::CloseTab => "Close tab",
            Command::NewNote => "New note",
            Command::Save => "Save",
            Command::TogglePreview => "Toggle edit/preview",
            Command::Find => "Find in note",
            Command::VaultSearch => "Search vault",
            Command::Settings => "Settings",
            Command::Quit => "Quit",
            Command::Shortcuts => "Shortcuts cheat sheet",
            Command::ThemePicker => "Theme picker",
            Command::OpenVault => "Open vault",
            Command::RecentVaults => "Recent vaults",
            Command::CloseVault => "Close vault",
            Command::FontBigger => "Larger editor font",
            Command::FontSmaller => "Smaller editor font",
            Command::FontReset => "Reset editor font",
            Command::NextTab => "Next tab",
            Command::PrevTab => "Previous tab",
            Command::FormerTab => "Former tab",
            Command::FocusFiles => "Focus file tree",
            Command::FocusOutline => "Focus outline",
            Command::NextPane => "Next pane",
            Command::PrevPane => "Previous pane",
            Command::Tab(_) => "Jump tab",
            Command::Escape => "Dismiss overlay",
            Command::Rename => "Rename",
            Command::NewFolder => "New folder",
            Command::DeleteItem => "Delete",
            Command::ToggleLeftSidebar => "Toggle files sidebar",
            Command::ToggleRightSidebar => "Toggle outline sidebar",
            Command::NavBack => "Navigate back",
            Command::NavForward => "Navigate forward",
            Command::RevealInTree => "Reveal note in tree",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Command::QuickOpen => "quick_open",
            Command::CommandPalette => "command_palette",
            Command::Graph => "graph",
            Command::CloseTab => "close_tab",
            Command::NewNote => "new_note",
            Command::Save => "save",
            Command::TogglePreview => "toggle_preview",
            Command::Find => "find",
            Command::VaultSearch => "vault_search",
            Command::Settings => "settings",
            Command::Quit => "quit",
            Command::Shortcuts => "shortcuts",
            Command::ThemePicker => "theme_picker",
            Command::OpenVault => "open_vault",
            Command::RecentVaults => "recent_vaults",
            Command::CloseVault => "close_vault",
            Command::FontBigger => "font_bigger",
            Command::FontSmaller => "font_smaller",
            Command::FontReset => "font_reset",
            Command::NextTab => "next_tab",
            Command::PrevTab => "prev_tab",
            Command::FormerTab => "former_tab",
            Command::FocusFiles => "focus_files",
            Command::FocusOutline => "focus_outline",
            Command::NextPane => "next_pane",
            Command::PrevPane => "prev_pane",
            Command::Tab(_) => "tab_n",
            Command::Escape => "escape",
            Command::Rename => "rename",
            Command::NewFolder => "new_folder",
            Command::DeleteItem => "delete_item",
            Command::ToggleLeftSidebar => "toggle_left_sidebar",
            Command::ToggleRightSidebar => "toggle_right_sidebar",
            Command::NavBack => "nav_back",
            Command::NavForward => "nav_forward",
            Command::RevealInTree => "reveal_in_tree",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileFormat {
    profile: Option<String>,
    bindings: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    pub profile: String,
    pub bindings: HashMap<Command, String>,
    shortcuts: HashMap<Command, KeyboardShortcut>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::obsidian_defaults()
    }
}

impl Keymap {
    pub fn obsidian_defaults() -> Self {
        let mut k = Self {
            profile: "obsidian".into(),
            bindings: HashMap::new(),
            shortcuts: HashMap::new(),
        };
        k.set(Command::QuickOpen, "Mod+O");
        k.set(Command::CommandPalette, "Mod+P");
        k.set(Command::Graph, "Mod+G");
        k.set(Command::CloseTab, "Mod+W");
        k.set(Command::NewNote, "Mod+N");
        k.set(Command::Save, "Mod+S");
        k.set(Command::TogglePreview, "Mod+E");
        k.set(Command::Find, "Mod+F");
        k.set(Command::VaultSearch, "Mod+Shift+F");
        k.set(Command::Settings, "Mod+,");
        k.set(Command::Quit, "Mod+Q");
        k.set(Command::Shortcuts, "?");
        k.set(Command::ThemePicker, "Mod+T");
        k.set(Command::OpenVault, "Mod+Shift+O");
        k.set(Command::RecentVaults, "Mod+R");
        k.set(Command::CloseVault, "Mod+Shift+W");
        k.set(Command::FontBigger, "Mod+=");
        k.set(Command::FontSmaller, "Mod+-");
        k.set(Command::FontReset, "Mod+0");
        k.set(Command::NextTab, "Alt+Right");
        k.set(Command::PrevTab, "Alt+Left");
        k.set(Command::FocusFiles, "Ctrl+Tab");
        k.set(Command::Escape, "Escape");
        k.set(Command::Rename, "F2");
        k.set(Command::NewFolder, "Mod+Shift+N");
        k.set(Command::DeleteItem, "Delete");
        k.set(Command::ToggleLeftSidebar, "Mod+B");
        k.set(Command::ToggleRightSidebar, "Mod+\\");
        k.set(Command::NavBack, "Mod+[");
        k.set(Command::NavForward, "Mod+]");
        k.set(Command::RevealInTree, "Mod+Shift+E");
        k.bindings
            .insert(Command::Shortcuts, "? / Mod+/".into());
        k
    }

    pub fn omarchy_defaults() -> Self {
        let mut k = Self::obsidian_defaults();
        k.profile = "omarchy".into();
        k.set(Command::FormerTab, "Ctrl+Alt+Tab");
        k.set(Command::FocusFiles, "Ctrl+Tab");
        k.set(Command::FocusOutline, "Ctrl+Shift+O");
        k.set(Command::NextPane, "Ctrl+Alt+Right");
        k.set(Command::PrevPane, "Ctrl+Alt+Left");
        for i in 1..=9u8 {
            k.set(Command::Tab(i), &format!("Ctrl+{i}"));
        }
        k
    }

    fn set(&mut self, cmd: Command, chord: &str) {
        self.bindings.insert(cmd, chord.to_string());
        if let Some(sc) = parse_chord(chord) {
            self.shortcuts.insert(cmd, sc);
        }
    }

    pub fn chord(&self, cmd: Command) -> String {
        self.bindings
            .get(&cmd)
            .cloned()
            .unwrap_or_else(|| "?".into())
    }

    pub fn display_chord(&self, cmd: Command) -> String {
        pretty_chord(&self.chord(cmd))
    }

    pub fn detect_and_load() -> Self {
        let prefer_omarchy = is_omarchy();
        let mut map = if prefer_omarchy {
            Self::omarchy_defaults()
        } else {
            Self::obsidian_defaults()
        };
        if let Some(path) = user_keybind_path() {
            if path.exists() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let _ = map.merge_toml(&text);
                }
            }
        } else if let Some(bundled) = bundled_profile_path(if prefer_omarchy {
            "omarchy"
        } else {
            "obsidian"
        }) {
            if let Ok(text) = std::fs::read_to_string(bundled) {
                let _ = map.merge_toml(&text);
            }
        }
        map
    }

    pub fn merge_toml(&mut self, text: &str) -> Result<(), String> {
        let file: FileFormat = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(p) = file.profile {
            self.profile = p;
        }
        for (k, v) in file.bindings {
            if let Some(cmd) = command_from_id(&k) {
                self.set(cmd, &v);
            } else if let Some(n) = k.strip_prefix("tab_") {
                if let Ok(i) = n.parse::<u8>() {
                    self.set(Command::Tab(i), &v);
                }
            }
        }
        Ok(())
    }

    pub fn consume(&self, ctx: &egui::Context, editor_focused: bool) -> Option<Command> {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            return Some(Command::Escape);
        }

        let pairs: Vec<(Command, KeyboardShortcut)> =
            self.shortcuts.iter().map(|(c, s)| (*c, *s)).collect();

        for (cmd, sc) in pairs {
            if cmd == Command::Shortcuts && editor_focused {
                let slash = KeyboardShortcut::new(Modifiers::COMMAND, Key::Slash);
                if ctx.input_mut(|i| i.consume_shortcut(&slash)) {
                    return Some(Command::Shortcuts);
                }
                continue;
            }
            // Don't steal Delete from the text editor.
            if cmd == Command::DeleteItem && editor_focused {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&sc)) {
                return Some(cmd);
            }
        }

        // Mod+= often arrives as Plus with shift on some layouts
        let plus = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Equals);
        if ctx.input_mut(|i| i.consume_shortcut(&plus)) {
            return Some(Command::FontBigger);
        }

        if !editor_focused
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Questionmark))
        {
            return Some(Command::Shortcuts);
        }
        if !editor_focused
            && ctx.input(|i| i.modifiers.shift)
            && ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Slash))
        {
            return Some(Command::Shortcuts);
        }

        None
    }

    pub fn all_listed(&self) -> Vec<(Command, String)> {
        let mut v: Vec<_> = self
            .bindings
            .iter()
            .filter(|(c, _)| !matches!(c, Command::Tab(_)))
            .map(|(c, s)| (*c, pretty_chord(s)))
            .collect();
        for i in 1..=9u8 {
            if let Some(s) = self.bindings.get(&Command::Tab(i)) {
                v.push((Command::Tab(i), pretty_chord(s)));
            }
        }
        v.sort_by(|a, b| a.1.cmp(&b.1));
        v
    }
}

fn command_from_id(id: &str) -> Option<Command> {
    Some(match id {
        "quick_open" => Command::QuickOpen,
        "command_palette" => Command::CommandPalette,
        "graph" => Command::Graph,
        "close_tab" => Command::CloseTab,
        "new_note" => Command::NewNote,
        "save" => Command::Save,
        "toggle_preview" => Command::TogglePreview,
        "find" => Command::Find,
        "vault_search" => Command::VaultSearch,
        "settings" => Command::Settings,
        "quit" => Command::Quit,
        "shortcuts" | "shortcuts_editor" => Command::Shortcuts,
        "theme_picker" => Command::ThemePicker,
        "open_vault" => Command::OpenVault,
        "recent_vaults" => Command::RecentVaults,
        "close_vault" => Command::CloseVault,
        "font_bigger" => Command::FontBigger,
        "font_smaller" => Command::FontSmaller,
        "font_reset" => Command::FontReset,
        "next_tab" => Command::NextTab,
        "prev_tab" => Command::PrevTab,
        "former_tab" => Command::FormerTab,
        "focus_files" => Command::FocusFiles,
        "focus_outline" => Command::FocusOutline,
        "next_pane" => Command::NextPane,
        "prev_pane" => Command::PrevPane,
        "escape" => Command::Escape,
        "rename" => Command::Rename,
        "new_folder" => Command::NewFolder,
        "delete_item" => Command::DeleteItem,
        "toggle_left_sidebar" => Command::ToggleLeftSidebar,
        "toggle_right_sidebar" => Command::ToggleRightSidebar,
        "nav_back" => Command::NavBack,
        "nav_forward" => Command::NavForward,
        "reveal_in_tree" => Command::RevealInTree,
        _ => return None,
    })
}

fn parse_chord(s: &str) -> Option<KeyboardShortcut> {
    let s = s.split(" / ").next()?.trim();
    if s.eq_ignore_ascii_case("escape") {
        return Some(KeyboardShortcut::new(Modifiers::NONE, Key::Escape));
    }
    if s == "?" {
        return None;
    }
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+') {
        let p = part.trim();
        match p.to_ascii_lowercase().as_str() {
            "mod" | "cmd" | "command" | "super" => mods.command = true,
            "ctrl" | "control" => mods.ctrl = true,
            "alt" | "option" => mods.alt = true,
            "shift" => mods.shift = true,
            "," => key = Some(Key::Comma),
            "/" => key = Some(Key::Slash),
            "-" | "minus" => key = Some(Key::Minus),
            "=" | "equals" | "plus" => key = Some(Key::Equals),
            "0" => key = Some(Key::Num0),
            "tab" => key = Some(Key::Tab),
            "left" => key = Some(Key::ArrowLeft),
            "right" => key = Some(Key::ArrowRight),
            "up" => key = Some(Key::ArrowUp),
            "down" => key = Some(Key::ArrowDown),
            "f2" => key = Some(Key::F2),
            "delete" | "del" => key = Some(Key::Delete),
            "backspace" => key = Some(Key::Backspace),
            "[" => key = Some(Key::OpenBracket),
            "]" => key = Some(Key::CloseBracket),
            "\\" | "backslash" => key = Some(Key::Backslash),
            other => {
                if other.len() == 1 {
                    let c = other.chars().next()?.to_ascii_uppercase();
                    key = Key::from_name(&c.to_string());
                } else {
                    key = Key::from_name(&other.to_ascii_uppercase());
                }
            }
        }
    }
    Some(KeyboardShortcut::new(mods, key?))
}

fn pretty_chord(s: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        s.replace("Mod+", "⌘")
            .replace("Ctrl+", "⌃")
            .replace("Alt+", "⌥")
            .replace("Shift+", "⇧")
    }
    #[cfg(not(target_os = "macos"))]
    {
        s.replace("Mod+", "^")
            .replace("Ctrl+", "^")
            .replace("Alt+", "⌥")
            .replace("Shift+", "⇧")
    }
}

pub fn is_omarchy() -> bool {
    PathBuf::from("/usr/share/omarchy").is_dir() || std::env::var_os("OMARCHY_PATH").is_some()
}

fn user_keybind_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("md", "Celestine", "celestine")?;
    Some(dirs.config_dir().join("keybindings.toml"))
}

fn bundled_profile_path(profile: &str) -> Option<PathBuf> {
    let name = format!("keybindings.{profile}.toml");
    let mut candidates = Vec::new();
    if let Some(proj) = directories::ProjectDirs::from("md", "Celestine", "celestine") {
        candidates.push(proj.data_dir().join("config").join(&name));
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".local/share/celestine/config")
                .join(&name),
        );
    }
    candidates.push(PathBuf::from("config").join(&name));
    candidates.push(PathBuf::from("../config").join(&name));
    candidates.push(PathBuf::from("../../config").join(&name));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config")
            .join(&name),
    );
    candidates.into_iter().find(|p| p.exists())
}

pub fn ensure_user_config() {
    if let Some(dirs) = directories::ProjectDirs::from("md", "Celestine", "celestine") {
        let _ = std::fs::create_dir_all(dirs.config_dir());
    }
}
