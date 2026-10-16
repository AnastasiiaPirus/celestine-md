use celestine_core::{resolve_link, ResolvedLink, VaultIndex};
use egui::{Color32, FontId, RichText, Sense, Stroke, TextStyle, Ui, Vec2};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::sync::OnceLock;

pub struct PreviewOutcome {
    pub open_rel: Option<String>,
    /// Heading text that was rendered this frame (for scroll targets).
    pub heading_rects: Vec<(usize, egui::Rect)>, // line index approx via counter
}

struct HeadingColors {
    h1: Color32,
    h2: Color32,
    h3: Color32,
    h4: Color32,
    h5: Color32,
    h6: Color32,
}

impl HeadingColors {
    fn for_level(&self, level: u8) -> Color32 {
        match level {
            1 => self.h1,
            2 => self.h2,
            3 => self.h3,
            4 => self.h4,
            5 => self.h5,
            _ => self.h6,
        }
    }

    fn from_theme(headings: &[Color32; 6]) -> Self {
        Self {
            h1: headings[0],
            h2: headings[1],
            h3: headings[2],
            h4: headings[3],
            h5: headings[4],
            h6: headings[5],
        }
    }
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!?\[\[([^\[\]]+)\]\]").expect("wikilink"))
}

/// Preprocess Obsidian-ish body: wikilinks + `:shortcode:` emoji expansion.
fn preprocess(body: &str) -> String {
    let body = crate::emoji_util::expand_shortcodes(body);
    let mut out = String::with_capacity(body.len() + 32);
    let mut last = 0;
    for cap in wikilink_re().captures_iter(&body) {
        let m = cap.get(0).unwrap();
        out.push_str(&body[last..m.start()]);
        let inner = cap.get(1).unwrap().as_str();
        let embed = m.as_str().starts_with('!');
        let (target, display) = match inner.split_once('|') {
            Some((t, d)) => (t.trim(), d.trim()),
            None => (inner.trim(), inner.trim()),
        };
        let target = target.split('#').next().unwrap_or(target);
        if embed {
            out.push_str(&format!("*[embed: {display}]* (`wikilink:{target}`)"));
        } else {
            out.push_str(&format!("[{display}](wikilink:{target})"));
        }
        last = m.end();
    }
    out.push_str(&body[last..]);
    out
}

/// Rich Markdown preview (tables, emphasis, headings by level, code, lists, blockquotes, tags).
pub fn show_preview(
    ui: &mut Ui,
    body: &str,
    note_rel: &str,
    index: &VaultIndex,
    accent: Color32,
    muted: Color32,
    text: Color32,
    font_size: f32,
    scroll_to_heading: Option<&str>,
    heading_palette: &[Color32; 6],
) -> PreviewOutcome {
    let headings = HeadingColors::from_theme(heading_palette);
    let prepared = preprocess(body);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(&prepared, options);
    let mut open_rel = None;
    let mut heading_rects = Vec::new();
    let mut heading_counter = 0usize;

    // Style state
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let code_span = false;
    let mut heading_level: Option<u8> = None;
    let mut link_dest: Option<String> = None;
    let mut list_depth: usize = 0;
    let mut ordered_list = false;
    let mut item_index: u32 = 0;
    let mut in_blockquote = false;
    let mut in_code_block = false;
    let mut code_buf = String::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut in_table = false;
    let mut in_table_cell = false;
    let mut cell_buf = String::new();
    let mut paragraph_buf: Vec<(RichText, Option<String>)> = Vec::new();

    let flush_paragraph = |ui: &mut Ui,
                           buf: &mut Vec<(RichText, Option<String>)>,
                           open: &mut Option<String>,
                           index: &VaultIndex,
                           note_rel: &str| {
        if buf.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (rt, dest) in buf.drain(..) {
                if let Some(d) = dest {
                    let resp = ui.add(egui::Label::new(rt).sense(Sense::click()));
                    if resp.clicked() {
                        if let Some(r) = resolve_dest(index, note_rel, &d) {
                            *open = Some(r);
                        }
                    }
                    resp.on_hover_text(d);
                } else {
                    ui.label(rt);
                }
            }
        });
        ui.add_space(4.0);
    };

    let style_text = |s: &str,
                      bold: bool,
                      italic: bool,
                      strike: bool,
                      code: bool,
                      heading: Option<u8>,
                      font_size: f32,
                      color: Color32,
                      headings: &HeadingColors|
     -> RichText {
        let mut size = font_size;
        let mut col = color;
        if let Some(level) = heading {
            col = headings.for_level(level);
            size = match level {
                1 => font_size * 1.75,
                2 => font_size * 1.45,
                3 => font_size * 1.25,
                4 => font_size * 1.12,
                _ => font_size * 1.05,
            };
        }
        let mut rt = RichText::new(s).size(size).color(col);
        if code {
            rt = rt.monospace().background_color(Color32::from_black_alpha(60));
        } else {
            rt = rt.family(egui::FontFamily::Proportional);
        }
        if bold || heading.is_some() {
            rt = rt.strong();
        }
        if italic {
            rt = rt.italics();
        }
        if strike {
            rt = rt.strikethrough();
        }
        rt
    };

    ui.vertical(|ui| {
        for event in parser {
            match event {
                Event::Start(Tag::Paragraph) => {
                    paragraph_buf.clear();
                }
                Event::End(TagEnd::Paragraph) => {
                    if in_blockquote {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("│ ").color(muted).size(font_size));
                            ui.vertical(|ui| {
                                flush_paragraph(
                                    ui,
                                    &mut paragraph_buf,
                                    &mut open_rel,
                                    index,
                                    note_rel,
                                );
                            });
                        });
                    } else {
                        flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    }
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    heading_level = Some(level as u8);
                    paragraph_buf.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    let level = heading_level.unwrap_or(1);
                    let plain: String = paragraph_buf
                        .iter()
                        .map(|(r, _)| r.text())
                        .collect::<Vec<_>>()
                        .join("");
                    let rt = style_text(
                        &plain,
                        true,
                        false,
                        false,
                        false,
                        Some(level),
                        font_size,
                        text,
                        &headings,
                    );
                    let resp = ui.add(egui::Label::new(rt).sense(Sense::click()));
                    heading_rects.push((heading_counter, resp.rect));
                    heading_counter += 1;
                    if let Some(want) = scroll_to_heading {
                        if plain.trim() == want.trim() {
                            resp.scroll_to_me(Some(egui::Align::TOP));
                        }
                    }
                    ui.add_space(6.0);
                    paragraph_buf.clear();
                    heading_level = None;
                }
                Event::Start(Tag::Strong) => bold = true,
                Event::End(TagEnd::Strong) => bold = false,
                Event::Start(Tag::Emphasis) => italic = true,
                Event::End(TagEnd::Emphasis) => italic = false,
                Event::Start(Tag::Strikethrough) => strike = true,
                Event::End(TagEnd::Strikethrough) => strike = false,
                Event::Code(code) => {
                    let rt = style_text(
                        &code,
                        bold,
                        italic,
                        strike,
                        true,
                        heading_level,
                        font_size,
                        text,
                        &headings,
                    );
                    paragraph_buf.push((rt, None));
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    link_dest = Some(dest_url.to_string());
                }
                Event::End(TagEnd::Link) => {
                    link_dest = None;
                }
                Event::Text(raw) => {
                    let s = raw.as_ref();
                    if in_code_block {
                        code_buf.push_str(s);
                        continue;
                    }
                    if in_table_cell {
                        cell_buf.push_str(s);
                        continue;
                    }
                    // Inline #tags
                    if s.starts_with('#')
                        && !s.contains(' ')
                        && heading_level.is_none()
                        && link_dest.is_none()
                    {
                        let rt = RichText::new(s).size(font_size).color(accent).strong();
                        paragraph_buf.push((rt, None));
                        continue;
                    }
                    let color = if link_dest.is_some() { accent } else { text };
                    let rt = style_text(
                        s,
                        bold,
                        italic,
                        strike,
                        code_span,
                        heading_level,
                        font_size,
                        color,
                        &headings,
                    );
                    paragraph_buf.push((rt, link_dest.clone()));
                }
                Event::SoftBreak | Event::HardBreak => {
                    if in_code_block {
                        code_buf.push('\n');
                    } else if in_table_cell {
                        cell_buf.push(' ');
                    } else {
                        paragraph_buf.push((
                            RichText::new(" ").size(font_size),
                            None,
                        ));
                    }
                }
                Event::Start(Tag::List(start)) => {
                    flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    list_depth += 1;
                    ordered_list = start.is_some();
                    item_index = start.unwrap_or(1) as u32;
                }
                Event::End(TagEnd::List(_)) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        ordered_list = false;
                    }
                    ui.add_space(2.0);
                }
                Event::Start(Tag::Item) => {
                    paragraph_buf.clear();
                }
                Event::End(TagEnd::Item) => {
                    let bullet = if ordered_list {
                        let n = item_index;
                        item_index += 1;
                        format!("{n}. ")
                    } else {
                        "• ".into()
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space((list_depth.saturating_sub(1) as f32) * 14.0);
                        ui.label(RichText::new(bullet).monospace().color(muted).size(font_size));
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (rt, dest) in paragraph_buf.drain(..) {
                            if let Some(d) = dest {
                                let resp = ui.add(egui::Label::new(rt).sense(Sense::click()));
                                if resp.clicked() {
                                    if let Some(r) = resolve_dest(index, note_rel, &d) {
                                        open_rel = Some(r);
                                    }
                                }
                            } else {
                                ui.label(rt);
                            }
                        }
                    });
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    in_blockquote = true;
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    in_blockquote = false;
                    ui.add_space(4.0);
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    in_code_block = true;
                    code_buf.clear();
                    let _ = kind;
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    let code = std::mem::take(&mut code_buf);
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(80))
                        .inner_margin(8)
                        .stroke(Stroke::new(1.0_f32, muted.gamma_multiply(0.4)))
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(code.trim_end())
                                        .monospace()
                                        .size(font_size * 0.92)
                                        .color(text),
                                )
                                .selectable(true),
                            );
                        });
                    ui.add_space(8.0);
                }
                Event::Start(Tag::Table(_)) => {
                    flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    in_table = true;
                    table_rows.clear();
                }
                Event::End(TagEnd::Table) => {
                    in_table = false;
                    if !table_rows.is_empty() {
                        let cols = table_rows.iter().map(|r| r.len()).max().unwrap_or(1);
                        egui::Grid::new(ui.scope_id().with("md-table"))
                            .striped(true)
                            .num_columns(cols)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                for (ri, row) in table_rows.iter().enumerate() {
                                    for cell in row {
                                        let rt = if ri == 0 {
                                            RichText::new(cell)
                                                .strong()
                                                .size(font_size)
                                                .color(accent)
                                        } else {
                                            RichText::new(cell).size(font_size).color(text)
                                        };
                                        ui.label(rt);
                                    }
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                    }
                    table_rows.clear();
                }
                Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                    table_row.clear();
                }
                Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                    if !table_row.is_empty() {
                        table_rows.push(std::mem::take(&mut table_row));
                    }
                }
                Event::Start(Tag::TableCell) => {
                    in_table_cell = true;
                    cell_buf.clear();
                }
                Event::End(TagEnd::TableCell) => {
                    in_table_cell = false;
                    table_row.push(std::mem::take(&mut cell_buf));
                }
                Event::Rule => {
                    flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
                    ui.add(egui::Separator::default().spacing(12.0));
                }
                Event::TaskListMarker(checked) => {
                    let mark = if checked { "☑ " } else { "☐ " };
                    paragraph_buf.push((
                        RichText::new(mark).size(font_size).color(accent),
                        None,
                    ));
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    // Strip simple tags; keep text-ish content
                    let stripped = html.replace('<', "").replace('>', "");
                    if !stripped.trim().is_empty() {
                        paragraph_buf.push((
                            RichText::new(stripped).size(font_size * 0.9).color(muted),
                            None,
                        ));
                    }
                }
                _ => {}
            }
        }
        flush_paragraph(ui, &mut paragraph_buf, &mut open_rel, index, note_rel);
    });

    let _ = (code_span, FontId::default(), TextStyle::Body, Vec2::ZERO, in_table);
    PreviewOutcome {
        open_rel,
        heading_rects,
    }
}

fn resolve_dest(index: &VaultIndex, note_rel: &str, dest: &str) -> Option<String> {
    let dest = dest.trim();
    let target = dest.strip_prefix("wikilink:").unwrap_or(dest);
    if target.starts_with("http://") || target.starts_with("https://") {
        return None;
    }
    match resolve_link(index, note_rel, target) {
        ResolvedLink::Note(id) => index.get(id).map(|n| n.rel.clone()),
        ResolvedLink::Missing { target } => Some(ensure_md(&target)),
        ResolvedLink::External { .. } => None,
    }
}

fn ensure_md(target: &str) -> String {
    if target.to_lowercase().ends_with(".md") {
        target.to_string()
    } else {
        format!("{target}.md")
    }
}
