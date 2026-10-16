use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::sync::Arc;

/// Install JetBrains Mono for UI/editor, with explicit emoji fallbacks so notes
/// can render Unicode emoji (and folder icons) instead of tofu boxes.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let regular = load_first(&[
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/jetbrainsmono/JetBrainsMono-Regular.ttf",
    ]);
    let bold = load_first(&[
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
        "/usr/share/fonts/jetbrainsmono/JetBrainsMono-Bold.ttf",
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

    // Bundled monochrome emoji (OFL / MIT) — always available, not color-CBDT.
    fonts.font_data.insert(
        "celestine_emoji".into(),
        Arc::new(
            FontData::from_static(include_bytes!("../../../assets/fonts/NotoEmoji-Regular.ttf"))
                .tweak(FontTweak {
                    // Match body size; default egui NotoEmoji tweak is ~0.81 (too small).
                    scale: 1.0,
                    ..Default::default()
                }),
        ),
    );
    fonts.font_data.insert(
        "celestine_emoji_icons".into(),
        Arc::new(
            FontData::from_static(include_bytes!("../../../assets/fonts/emoji-icon-font.ttf")).tweak(
                FontTweak {
                    scale: 0.95,
                    ..Default::default()
                },
            ),
        ),
    );

    // Family order: primary text font, then emoji fonts, then remaining defaults.
    // Per-glyph fallback skips fonts that lack the codepoint.
    rebuild_family(&mut fonts, FontFamily::Proportional);
    rebuild_family(&mut fonts, FontFamily::Monospace);

    ctx.set_fonts(fonts);
}

fn rebuild_family(fonts: &mut FontDefinitions, family: FontFamily) {
    let mut ordered: Vec<String> = Vec::new();
    if fonts.font_data.contains_key("celestine_mono") {
        ordered.push("celestine_mono".into());
    }
    ordered.push("celestine_emoji".into());
    ordered.push("celestine_emoji_icons".into());
    // Keep egui built-ins as further fallback (√, rare scripts, etc.)
    if let Some(existing) = fonts.families.get(&family).cloned() {
        for name in existing {
            if !ordered.iter().any(|n| n == &name) {
                ordered.push(name);
            }
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
