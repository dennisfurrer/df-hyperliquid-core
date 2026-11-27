//! The results as a PDF: a dark title band with the machine, the headline ratios as tiles, then
//! every table with the best figure of each row marked, and the notes. A4, one or more pages.

use serde_json::Value;

use crate::offline::{DF, DF_SECP, HYPERSDK};
use crate::pdf::{self, Font, PAGE_HEIGHT, PAGE_WIDTH, Page, Rgb};
use crate::report::{Table, heading, tables};

const MARGIN: f64 = 40.0;
const CONTENT: f64 = PAGE_WIDTH - 2.0 * MARGIN;
const BOTTOM: f64 = PAGE_HEIGHT - 56.0;

const INK: Rgb = Rgb(15, 23, 42);
const MUTED: Rgb = Rgb(100, 116, 139);
const RULE: Rgb = Rgb(226, 232, 240);
const HEAD_FILL: Rgb = Rgb(241, 245, 249);
const ZEBRA: Rgb = Rgb(248, 250, 252);
const BAND: Rgb = Rgb(11, 16, 32);
const BAND_TEXT: Rgb = Rgb(203, 213, 225);
const ACCENT: Rgb = Rgb(45, 216, 156);
const BEST: Rgb = Rgb(21, 128, 61);
const TILE: Rgb = Rgb(240, 253, 248);
const WARN: Rgb = Rgb(220, 38, 38);

/// Pages filled from the top, with a cursor `y` on the current one.
struct Layout {
    pages: Vec<Page>,
    y: f64,
}

impl Layout {
    fn page(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.pages.push(Page::default());
        }
        let last = self.pages.len() - 1;
        &mut self.pages[last]
    }

    /// Starts a new page when fewer than `needed` points are left on this one.
    fn room(&mut self, needed: f64) {
        if self.y + needed > BOTTOM {
            self.pages.push(Page::default());
            self.y = MARGIN;
        }
    }

    fn paragraph(&mut self, text: &str, font: Font, size: f64, color: Rgb) {
        for line in pdf::wrap(text, font, size, CONTENT) {
            self.room(size * 1.5);
            self.y += size * 1.45;
            let y = self.y;
            self.page().text(MARGIN, y, font, size, color, &line);
        }
    }
}

fn band(layout: &mut Layout, result: &Value) {
    let quick = result["options"]["quick"].as_bool() == Some(true);
    let page = layout.page();
    page.rect(0.0, 0.0, PAGE_WIDTH, 118.0, BAND);
    page.rect(0.0, 118.0, PAGE_WIDTH, 3.0, ACCENT);
    page.text(MARGIN, 52.0, Font::Bold, 24.0, Rgb(255, 255, 255), "df-hyperliquid-core");
    page.text(MARGIN, 74.0, Font::Regular, 12.0, BAND_TEXT, "Benchmark results");
    let cpu = result["machine"]["cpu"].as_str().unwrap_or("unknown CPU");
    let os = result["machine"]["os"].as_str().unwrap_or_default();
    let arch = result["machine"]["arch"].as_str().unwrap_or_default();
    page.text(MARGIN, 100.0, Font::Regular, 9.0, ACCENT, &format!("{cpu}  |  {os}  |  {arch}"));
    let started = result["started_utc"].as_str().unwrap_or_default().replace('T', " ").replace('Z', " UTC");
    page.text_right(PAGE_WIDTH - MARGIN, 52.0, Font::Regular, 9.0, BAND_TEXT, &started);
    let mode = match (result["options"]["compare"].as_bool() == Some(true), result["options"]["network"].as_bool() == Some(true)) {
        (true, true) => "compare, offline and network",
        (true, false) => "compare, offline",
        (false, true) => "df-hyperliquid-core alone, offline and network",
        (false, false) => "df-hyperliquid-core alone, offline",
    };
    page.text_right(PAGE_WIDTH - MARGIN, 68.0, Font::Regular, 9.0, BAND_TEXT, mode);
    if quick {
        let label = "QUICK RUN: NOT FOR PUBLISHING";
        let w = pdf::width(label, Font::Bold, 8.0) + 16.0;
        page.rect(PAGE_WIDTH - MARGIN - w, 82.0, w, 18.0, WARN);
        page.text_right(PAGE_WIDTH - MARGIN - 8.0, 94.5, Font::Bold, 8.0, Rgb(255, 255, 255), label);
    }
    layout.y = 121.0 + 24.0;
}

/// The headline figures: ratios against hypersdk in compare mode, our own figures otherwise.
fn tiles(layout: &mut Layout, result: &Value) {
    let f = |value: &Value| value.as_f64().unwrap_or(0.0);
    let sign = |name: &str| f(&result["signing"][name]["sign"]["median_ns"]);
    let parse = |name: &str, part: &str| f(&result["parsing"][name]["parse"][part]["frames_per_s"]);
    let mut items: Vec<(String, String)> = Vec::new();
    if result["parsing"].get(HYPERSDK).is_some() && parse(HYPERSDK, "all") > 0.0 && sign(DF) > 0.0 {
        items.push((format!("{:.1}x", parse(DF, "all") / parse(HYPERSDK, "all")), "websocket parsing vs hypersdk".to_owned()));
        items.push((format!("{:.1}x", parse(DF, "l2Book") / parse(HYPERSDK, "l2Book")), "l2Book parsing vs hypersdk".to_owned()));
        items.push((format!("{:.1}x", sign(HYPERSDK) / sign(DF)), "order signing vs hypersdk".to_owned()));
        if sign(DF_SECP) > 0.0 {
            items.push((format!("{:.1}x", sign(HYPERSDK) / sign(DF_SECP)), "signing with secp256k1 vs hypersdk".to_owned()));
        }
    } else {
        items.push((format!("{:.1} us", sign(DF) / 1_000.0), "hash and sign one order".to_owned()));
        if sign(DF_SECP) > 0.0 {
            items.push((format!("{:.1} us", sign(DF_SECP) / 1_000.0), "with the secp256k1 feature".to_owned()));
        }
        items.push((format!("{:.2}M", parse(DF, "all") / 1e6), "websocket frames a second".to_owned()));
    }
    let gap = 10.0;
    let w = (CONTENT - gap * (items.len() as f64 - 1.0)) / items.len() as f64;
    let y = layout.y;
    let page = layout.page();
    for (at, (figure, label)) in items.iter().enumerate() {
        let x = MARGIN + at as f64 * (w + gap);
        page.rect(x, y, w, 66.0, TILE);
        page.rect(x, y, 3.0, 66.0, ACCENT);
        page.text(x + 14.0, y + 34.0, Font::Bold, 24.0, BEST, figure);
        for (line_at, line) in pdf::wrap(label, Font::Regular, 8.0, w - 22.0).iter().enumerate().take(2) {
            page.text(x + 14.0, y + 49.0 + line_at as f64 * 10.0, Font::Regular, 8.0, MUTED, line);
        }
    }
    layout.y += 66.0 + 18.0;
}

fn table(layout: &mut Layout, table: &Table) {
    let columns = table.header.len();
    let first = if columns > 5 { 150.0 } else { 175.0 };
    let rest = (CONTENT - first) / (columns - 1) as f64;
    let x_of = |column: usize| if column == 0 { MARGIN } else { MARGIN + first + (column - 1) as f64 * rest };
    let head_lines: Vec<Vec<String>> = table
        .header
        .iter()
        .enumerate()
        .map(|(at, text)| pdf::wrap(text, Font::Bold, 7.5, if at == 0 { first - 12.0 } else { rest - 10.0 }))
        .collect();
    let head_height = 10.0 + 9.0 * head_lines.iter().map(Vec::len).max().unwrap_or(1) as f64;
    let row_height = 17.0;
    layout.room(22.0 + head_height + row_height * table.rows.len().min(4) as f64);

    layout.y += 14.0;
    let y = layout.y;
    layout.page().text(MARGIN, y, Font::Bold, 11.0, INK, &table.title);
    layout.y += 8.0;

    let top = layout.y;
    let page = layout.page();
    page.rect(MARGIN, top, CONTENT, head_height, HEAD_FILL);
    for (column, lines) in head_lines.iter().enumerate() {
        for (line_at, line) in lines.iter().enumerate() {
            let y = top + 12.0 + line_at as f64 * 9.0;
            if column == 0 {
                page.text(x_of(0) + 8.0, y, Font::Bold, 7.5, MUTED, line);
            } else {
                page.text_right(x_of(column) + rest - 8.0, y, Font::Bold, 7.5, MUTED, line);
            }
        }
    }
    layout.y += head_height;

    for (row_at, (cells, best)) in table.rows.iter().enumerate() {
        layout.room(row_height);
        let top = layout.y;
        let page = layout.page();
        if row_at % 2 == 1 {
            page.rect(MARGIN, top, CONTENT, row_height, ZEBRA);
        }
        for (column, cell) in cells.iter().enumerate() {
            let y = top + 11.5;
            if column == 0 {
                page.text(x_of(0) + 8.0, y, Font::Regular, 8.0, INK, cell);
            } else if Some(column) == *best {
                page.text_right(x_of(column) + rest - 8.0, y, Font::Bold, 8.0, BEST, cell);
            } else {
                page.text_right(x_of(column) + rest - 8.0, y, Font::Regular, 8.0, INK, cell);
            }
        }
        layout.y += row_height;
    }
    let y = layout.y;
    layout.page().line(MARGIN, y, MARGIN + CONTENT, y, RULE, 0.75);
    if let Some(note) = &table.note {
        layout.y += 2.0;
        layout.paragraph(note, Font::Regular, 7.5, MUTED);
    }
    layout.y += 8.0;
}

pub fn render(result: &Value) -> Vec<u8> {
    let mut layout = Layout { pages: Vec::new(), y: MARGIN };
    band(&mut layout, result);
    tiles(&mut layout, result);
    for line in heading(result) {
        let warn = line.starts_with("QUICK RUN");
        layout.paragraph(&line, if warn { Font::Bold } else { Font::Regular }, 8.0, if warn { WARN } else { MUTED });
    }
    layout.y += 4.0;
    for item in tables(result) {
        table(&mut layout, &item);
    }
    if let Some(skipped) = result["skipped"].as_array().filter(|skipped| !skipped.is_empty()) {
        layout.room(40.0);
        layout.y += 14.0;
        let y = layout.y;
        layout.page().text(MARGIN, y, Font::Bold, 11.0, INK, "Skipped");
        for note in skipped {
            layout.paragraph(&format!("- {}", note.as_str().unwrap_or_default()), Font::Regular, 8.0, MUTED);
        }
    }
    let count = layout.pages.len();
    for (at, page) in layout.pages.iter_mut().enumerate() {
        page.line(MARGIN, PAGE_HEIGHT - 40.0, PAGE_WIDTH - MARGIN, PAGE_HEIGHT - 40.0, RULE, 0.75);
        page.text(MARGIN, PAGE_HEIGHT - 26.0, Font::Regular, 7.5, MUTED, "Method and how to rerun: github.com/dennisfurrer/df-hyperliquid-core/tree/main/bench");
        page.text_right(PAGE_WIDTH - MARGIN, PAGE_HEIGHT - 26.0, Font::Regular, 7.5, MUTED, &format!("{} / {count}", at + 1));
    }
    let title = format!("df-hyperliquid-core benchmark, {}", result["machine"]["cpu"].as_str().unwrap_or_default());
    pdf::document(&title, &layout.pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_standard_metrics() {
        assert!((pdf::width("0123456789", Font::Regular, 10.0) - 55.6).abs() < 1e-9);
        assert!((pdf::width("abc", Font::Bold, 10.0) - 17.23).abs() < 1e-9);
    }

    #[test]
    fn a_document_has_a_header_an_xref_and_an_end() {
        let bytes = pdf::document("t", &[Page::default()]);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("xref") && text.trim_end().ends_with("%%EOF"));
        let _ = DF;
    }
}
