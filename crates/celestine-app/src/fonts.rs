use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Install JetBrains Mono for UI/editor text.
///
/// Color emoji (and other missing scripts) come from eframe's `system_fonts`
/// provider (`Noto Color Emoji` on this machine). We deliberately do **not**
/// install monochrome outline emoji fonts — those would claim the same
/// codepoints and paint as washed-out wireframe glyphs instead of color.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // Prefer plain JetBrains Mono (not Nerd) so PUA/icon patches never shadow text.
    let regular = load_first(&[
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/jetbrainsmono/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/OTF/JetBrainsMono-Regular.otf",
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMonoNL-Regular.ttf",
    ]);
    let bold = load_first(&[
        "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
        "/usr/share/fonts/jetbrainsmono/JetBrainsMono-Bold.ttf",
        "/usr/share/fonts/OTF/JetBrainsMono-Bold.otf",
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf",
    ]);

    if let Some(data) = regular {
        fonts.font_data.insert(
            "celestine_mono".into(),
            Arc::new(FontData::from_owned(data)),
        );
    }
    if let Some(data) = bold {
        fonts.font_data.insert(
            "celestine_mono_bold".into(),
            Arc::new(FontData::from_owned(data)),
        );
        fonts.families.insert(
            FontFamily::Name("celestine_mono_bold".into()),
            vec!["celestine_mono_bold".into()],
        );
    }

    // Drop stock monochrome emoji faces so the system color-emoji provider
    // owns those codepoints (otherwise OpenMoji/Noto outline wins and looks flat).
    let emoji_keys: Vec<String> = fonts
        .font_data
        .keys()
        .filter(|k| {
            let l = k.to_ascii_lowercase();
            l.contains("emoji") || l.contains("openmoji")
        })
        .cloned()
        .collect();
    for key in emoji_keys {
        fonts.font_data.remove(&key);
    }
    strip_emoji_faces_from_families(&mut fonts);

    prepend_mono(&mut fonts, FontFamily::Proportional);
    prepend_mono(&mut fonts, FontFamily::Monospace);

    ctx.set_fonts(fonts);
}

fn strip_emoji_faces_from_families(fonts: &mut FontDefinitions) {
    for family in fonts.families.values_mut() {
        family.retain(|name| {
            let l = name.to_ascii_lowercase();
            !l.contains("emoji") && !l.contains("openmoji")
        });
    }
}

fn prepend_mono(fonts: &mut FontDefinitions, family: FontFamily) {
    let Some(existing) = fonts.families.get(&family).cloned() else {
        if fonts.font_data.contains_key("celestine_mono") {
            fonts
                .families
                .insert(family, vec!["celestine_mono".into()]);
        }
        return;
    };
    let mut ordered = Vec::new();
    if fonts.font_data.contains_key("celestine_mono") {
        ordered.push("celestine_mono".into());
    }
    for name in existing {
        if !ordered.iter().any(|n| n == &name) {
            ordered.push(name);
        }
    }
    fonts.families.insert(family, ordered);
}

fn load_first(paths: &[&str]) -> Option<Vec<u8>> {
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(bytes);
        }
    }
    None
}
