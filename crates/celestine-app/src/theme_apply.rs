use celestine_theme::ThemeTokens;
use egui::style::WidgetVisuals;
use egui::{
    Color32, FontFamily, FontId, Frame, Margin, Rounding, Shadow, Stroke, Style, TextStyle, Visuals,
};

pub fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
        (a.a() as f32 * inv + b.a() as f32 * t) as u8,
    )
}

pub fn is_dark(t: &ThemeTokens) -> bool {
    luminance(t.background_primary) < 0.5
}

/// Sidebars / chrome — slightly lifted charcoal (flea rail).
pub fn panel_bg(t: &ThemeTokens) -> Color32 {
    color(t.background_secondary)
}

/// Editor / reading surface — darker than side panels for easy reading.
pub fn content_bg(t: &ThemeTokens) -> Color32 {
    let panel = panel_bg(t);
    let primary = color(t.background_primary);
    if is_dark(t) {
        let dark = mix(primary, Color32::BLACK, 0.55);
        if luma(dark) < luma(panel) {
            dark
        } else {
            mix(panel, Color32::BLACK, 0.45)
        }
    } else {
        let light = mix(primary, Color32::WHITE, 0.35);
        if luma(light) > luma(panel) {
            light
        } else {
            mix(panel, Color32::WHITE, 0.25)
        }
    }
}

/// Apply flea-like flat chrome: naked buttons, hairline rules, JetBrains sizes.
pub fn apply_theme(ctx: &egui::Context, t: &ThemeTokens, ui_font_scale: f32) {
    let dark = is_dark(t);
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let content = content_bg(t);
    let panel = panel_bg(t);
    let text = color(t.text_normal);
    let muted = color(t.text_muted);
    let accent = color(t.interactive_accent);
    let border = color(t.background_modifier_border);
    let sel = color(t.selection);

    visuals.window_fill = content;
    visuals.panel_fill = panel;
    visuals.extreme_bg_color = content;
    visuals.faint_bg_color = mix(panel, content, 0.5);
    visuals.code_bg_color = mix(content, panel, 0.35);
    visuals.override_text_color = Some(text);
    visuals.selection.bg_fill = sel;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent);
    visuals.hyperlink_color = accent;
    visuals.window_stroke = Stroke::NONE;
    visuals.window_rounding = Rounding::ZERO;
    visuals.menu_rounding = Rounding::same(2.0);
    visuals.window_shadow = Shadow::NONE;
    visuals.popup_shadow = Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 12.0,
        spread: 0.0,
        color: Color32::from_black_alpha(if dark { 80 } else { 30 }),
    };

    // Naked controls: no fills, no borders — hover just brightens text.
    let naked = |fg: Color32| WidgetVisuals {
        bg_fill: Color32::TRANSPARENT,
        weak_bg_fill: Color32::TRANSPARENT,
        bg_stroke: Stroke::NONE,
        rounding: Rounding::ZERO,
        fg_stroke: Stroke::new(1.0_f32, fg),
        expansion: 0.0,
    };

    visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: panel,
        weak_bg_fill: Color32::TRANSPARENT,
        bg_stroke: Stroke::new(1.0_f32, border.gamma_multiply(0.6)),
        rounding: Rounding::ZERO,
        fg_stroke: Stroke::new(1.0_f32, muted),
        expansion: 0.0,
    };
    visuals.widgets.inactive = naked(text);
    visuals.widgets.hovered = naked(accent);
    visuals.widgets.active = naked(accent);
    visuals.widgets.open = WidgetVisuals {
        bg_fill: sel,
        weak_bg_fill: sel,
        bg_stroke: Stroke::NONE,
        rounding: Rounding::ZERO,
        fg_stroke: Stroke::new(1.0_f32, accent),
        expansion: 0.0,
    };

    let s = ui_font_scale.clamp(0.85, 1.6);
    let mut style = Style {
        visuals,
        ..(*ctx.style()).clone()
    };
    // Compact flea spacing
    style.spacing.item_spacing = egui::vec2(6.0 * s, 3.0 * s);
    style.spacing.window_margin = Margin::symmetric(8.0 * s, 6.0 * s);
    style.spacing.menu_margin = Margin::symmetric(8.0 * s, 6.0 * s);
    style.spacing.button_padding = egui::vec2(4.0 * s, 2.0 * s);
    style.spacing.indent = 14.0 * s;
    // Solid scrollbars sit on the far right of the note pane (not floating over text).
    style.spacing.scroll = egui::style::ScrollStyle::solid();

    // Flea body ~14px at scale 1.0 (shell.toml base-size 14)
    let body = 13.0 * s;
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(body * 0.85, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(body, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(body, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(body * 1.15, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(body, FontFamily::Monospace),
    );

    ctx.set_style(style);
}

pub fn overlay_frame(t: &ThemeTokens) -> Frame {
    // Lifted, fully opaque surface so the modal stays readable over the dimmer.
    let panel = panel_bg(t);
    let fill = if is_dark(t) {
        mix(panel, Color32::WHITE, 0.06)
    } else {
        mix(panel, Color32::WHITE, 0.55)
    };
    let border = color(t.background_modifier_border);
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border.gamma_multiply(0.85)))
        .rounding(Rounding::same(2.0))
        .inner_margin(Margin::symmetric(14.0, 10.0))
        .shadow(Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: Color32::from_black_alpha(if is_dark(t) { 160 } else { 50 }),
        })
}

pub fn panel_frame(ctx: &egui::Context) -> Frame {
    Frame::side_top_panel(&ctx.style())
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .stroke(Stroke::NONE)
}

pub fn content_frame(t: &ThemeTokens) -> Frame {
    Frame::none()
        .fill(content_bg(t))
        .inner_margin(Margin::symmetric(12.0, 8.0))
}

/// Glyph / text control with no chrome (flea ChromeButton).
pub fn naked_button(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Button::new(label).frame(false))
}

pub fn naked_icon(
    ui: &mut egui::Ui,
    name: &str,
    active: bool,
    accent: Color32,
    muted: Color32,
) -> egui::Response {
    let color = if active { accent } else { muted };
    crate::icons::button(ui, name, 16.0, color)
}

pub fn sidebar_toggle(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    accent: Color32,
    muted: Color32,
) -> egui::Response {
    // Flea-style: simple text, no box; dim when hidden.
    let color = if active { accent } else { muted };
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .monospace()
                .size(12.5)
                .color(color),
        )
        .frame(false),
    )
}

fn luminance(rgb: [u8; 3]) -> f32 {
    luma(color(rgb))
}

fn luma(c: Color32) -> f32 {
    0.2126 * (c.r() as f32 / 255.0)
        + 0.7152 * (c.g() as f32 / 255.0)
        + 0.0722 * (c.b() as f32 / 255.0)
}
