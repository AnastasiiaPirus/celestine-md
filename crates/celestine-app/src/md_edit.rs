use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, FontFamily, FontId, Galley, Ui};
use std::sync::Arc;

/// Flea-adjacent heading palette (accent + soft secondary hues).
pub fn heading_color(level: u8, accent: Color32, text: Color32) -> Color32 {
    match level {
        1 => accent,
        2 => Color32::from_rgb(0x7a, 0xc2, 0xf0),
        3 => Color32::from_rgb(0xb4, 0x9d, 0xeb),
        4 => Color32::from_rgb(0x9e, 0xce, 0x6a),
        5 => Color32::from_rgb(0xe0, 0xaf, 0x68),
        _ => text.gamma_multiply(0.85),
    }
}

/// Syntax-tint Markdown source while editing (headings, markers, wikilinks, code).
pub fn layout(
    ui: &Ui,
    text: &str,
    wrap_width: f32,
    font_size: f32,
    text_color: Color32,
    muted: Color32,
    accent: Color32,
) -> Arc<Galley> {
    // Infinite/NaN wrap width disables soft-wrap and causes mid-glyph clipping
    // when the note column is narrower than a source line (sidebars + small window).
    let max_width = if wrap_width.is_finite() && wrap_width > 1.0 {
        wrap_width
    } else {
        1.0
    };

    let mut job = LayoutJob::default();
    job.wrap = TextWrapping {
        max_width,
        // Long tokens (URLs, paths) must still break inside a narrow column.
        break_anywhere: true,
        ..Default::default()
    };
    job.break_on_newline = true;

    let base = FontId::new(font_size, FontFamily::Monospace);
    let strong = FontId::new(font_size, FontFamily::Monospace);

    for line in text.split_inclusive('\n') {
        paint_line(&mut job, line, &base, &strong, text_color, muted, accent);
    }
    if text.is_empty() {
        job.append(
            "",
            0.0,
            TextFormat {
                font_id: base,
                color: text_color,
                ..Default::default()
            },
        );
    }

    ui.fonts(|f| f.layout_job(job))
}

fn paint_line(
    job: &mut LayoutJob,
    line: &str,
    base: &FontId,
    strong: &FontId,
    text_color: Color32,
    muted: Color32,
    accent: Color32,
) {
    let trimmed = line.trim_end_matches('\n');
    let newline = if line.ends_with('\n') { "\n" } else { "" };

    // ATX headings
    if let Some(level) = heading_level(trimmed) {
        let hashes = &trimmed[..level as usize];
        let after = &trimmed[level as usize..];
        let color = heading_color(level, accent, text_color);
        let size = match level {
            1 => base.size * 1.35,
            2 => base.size * 1.22,
            3 => base.size * 1.12,
            _ => base.size,
        };
        job.append(
            hashes,
            0.0,
            TextFormat {
                font_id: FontId::new(size, FontFamily::Monospace),
                color: color.gamma_multiply(0.7),
                ..Default::default()
            },
        );
        job.append(
            after,
            0.0,
            TextFormat {
                font_id: FontId::new(size, FontFamily::Monospace),
                color,
                ..Default::default()
            },
        );
        append_nl(job, newline, base, text_color);
        return;
    }

    // Blockquote
    if trimmed.starts_with('>') {
        job.append(
            trimmed,
            0.0,
            TextFormat {
                font_id: base.clone(),
                color: muted,
                italics: true,
                ..Default::default()
            },
        );
        append_nl(job, newline, base, text_color);
        return;
    }

    // Horizontal rule
    if is_hr(trimmed) {
        job.append(
            trimmed,
            0.0,
            TextFormat {
                font_id: base.clone(),
                color: muted.gamma_multiply(0.7),
                ..Default::default()
            },
        );
        append_nl(job, newline, base, text_color);
        return;
    }

    // List / task prefix
    let (prefix, body) = split_list_prefix(trimmed);
    if !prefix.is_empty() {
        job.append(
            prefix,
            0.0,
            TextFormat {
                font_id: base.clone(),
                color: muted,
                ..Default::default()
            },
        );
        paint_inline(job, body, base, strong, text_color, muted, accent);
        append_nl(job, newline, base, text_color);
        return;
    }

    paint_inline(job, trimmed, base, strong, text_color, muted, accent);
    append_nl(job, newline, base, text_color);
}

fn append_nl(job: &mut LayoutJob, newline: &str, base: &FontId, text_color: Color32) {
    if !newline.is_empty() {
        job.append(
            newline,
            0.0,
            TextFormat {
                font_id: base.clone(),
                color: text_color,
                ..Default::default()
            },
        );
    }
}

fn heading_level(line: &str) -> Option<u8> {
    if !line.starts_with('#') {
        return None;
    }
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if rest.is_empty() || rest.starts_with(' ') {
        Some(level as u8)
    } else {
        None
    }
}

fn is_hr(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3
        && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

fn split_list_prefix(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    let rest = &line[i..];
    // Tasks before plain bullets so "- [ ] " isn't eaten as "- "
    let patterns: &[(usize, bool)] = &[
        (
            6,
            rest.starts_with("- [ ] ")
                || rest.starts_with("* [ ] ")
                || rest.starts_with("- [x] ")
                || rest.starts_with("- [X] ")
                || rest.starts_with("* [x] ")
                || rest.starts_with("* [X] "),
        ),
        (
            2,
            rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ "),
        ),
    ];
    for &(pat_len, ok) in patterns {
        if ok {
            let end = i + pat_len.min(rest.len());
            return (&line[..end], &line[end..]);
        }
    }
    // ordered list "1. "
    if let Some(dot) = rest.find(". ") {
        if rest[..dot].bytes().all(|b| b.is_ascii_digit()) && dot > 0 {
            let end = i + dot + 2;
            return (&line[..end], &line[end..]);
        }
    }
    ("", line)
}

fn paint_inline(
    job: &mut LayoutJob,
    text: &str,
    base: &FontId,
    strong: &FontId,
    text_color: Color32,
    muted: Color32,
    accent: Color32,
) {
    let mut i = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();

    while i < n {
        let (byte_i, ch) = chars[i];

        // Wikilink [[...]]
        if ch == '[' && i + 1 < n && chars[i + 1].1 == '[' {
            if let Some(end) = find_closing_wikilink(&chars, i) {
                let start_b = byte_i;
                let end_b = chars[end].0 + chars[end].1.len_utf8();
                job.append(
                    &text[start_b..end_b],
                    0.0,
                    TextFormat {
                        font_id: base.clone(),
                        color: accent,
                        ..Default::default()
                    },
                );
                i = end + 1;
                continue;
            }
        }

        // Inline code `...`
        if ch == '`' {
            if let Some(end) = find_char(&chars, i + 1, '`') {
                let start_b = byte_i;
                let end_b = chars[end].0 + 1;
                job.append(
                    &text[start_b..end_b],
                    0.0,
                    TextFormat {
                        font_id: base.clone(),
                        color: muted,
                        background: Color32::from_black_alpha(50),
                        ..Default::default()
                    },
                );
                i = end + 1;
                continue;
            }
        }

        // Bold **...**
        if ch == '*' && i + 1 < n && chars[i + 1].1 == '*' {
            if let Some(end) = find_double_star(&chars, i + 2) {
                let start_b = byte_i;
                let end_b = chars[end + 1].0 + 1;
                job.append(
                    &text[start_b..end_b],
                    0.0,
                    TextFormat {
                        font_id: strong.clone(),
                        color: text_color,
                        ..Default::default()
                    },
                );
                i = end + 2;
                continue;
            }
        }

        // Tags #word
        if ch == '#' && (i == 0 || chars[i - 1].1.is_whitespace()) {
            let mut j = i + 1;
            while j < n && (chars[j].1.is_alphanumeric() || matches!(chars[j].1, '_' | '/' | '-'))
            {
                j += 1;
            }
            if j > i + 1 {
                let end_b = if j < n {
                    chars[j].0
                } else {
                    text.len()
                };
                job.append(
                    &text[byte_i..end_b],
                    0.0,
                    TextFormat {
                        font_id: base.clone(),
                        color: accent,
                        ..Default::default()
                    },
                );
                i = j;
                continue;
            }
        }

        // Plain run until next special
        let mut j = i + 1;
        while j < n {
            let c = chars[j].1;
            if c == '`' || c == '[' || c == '*' || c == '#' {
                break;
            }
            j += 1;
        }
        let end_b = if j < n { chars[j].0 } else { text.len() };
        job.append(
            &text[byte_i..end_b],
            0.0,
            TextFormat {
                font_id: base.clone(),
                color: text_color,
                ..Default::default()
            },
        );
        i = j;
    }
}

fn find_char(chars: &[(usize, char)], from: usize, needle: char) -> Option<usize> {
    chars[from..].iter().position(|(_, c)| *c == needle).map(|p| from + p)
}

fn find_closing_wikilink(chars: &[(usize, char)], start: usize) -> Option<usize> {
    // start at first '[' of [[
    let mut i = start + 2;
    while i + 1 < chars.len() {
        if chars[i].1 == ']' && chars[i + 1].1 == ']' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn find_double_star(chars: &[(usize, char)], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < chars.len() {
        if chars[i].1 == '*' && chars[i + 1].1 == '*' {
            return Some(i);
        }
        i += 1;
    }
    None
}
