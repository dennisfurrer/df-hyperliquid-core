//! A small PDF writer: A4 pages of filled rectangles, lines and text in two of the standard fonts
//! every PDF reader has (Helvetica and Helvetica-Bold), so nothing is embedded and the
//! runner needs no dependency. Coordinates are points from the top-left corner. Text is ASCII;
//! anything else is written as `?`.

use std::fmt::Write as _;

pub const PAGE_WIDTH: f64 = 595.28;
pub const PAGE_HEIGHT: f64 = 841.89;

#[derive(Clone, Copy)]
pub enum Font {
    Regular,
    Bold,
}

impl Font {
    const fn resource(self) -> &'static str {
        match self {
            Font::Regular => "F1",
            Font::Bold => "F2",
        }
    }
}

/// An RGB colour, 0 to 255 a channel.
#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn ops(self) -> String {
        format!("{:.3} {:.3} {:.3}", f64::from(self.0) / 255.0, f64::from(self.1) / 255.0, f64::from(self.2) / 255.0)
    }
}

/// Glyph widths in thousandths of the font size for characters 32 to 126, from the standard
/// Adobe font metrics.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556,
    556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334,
    260, 334, 584,
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611,
    556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389,
    280, 389, 584,
];

fn ascii(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.chars().map(|c| if (' '..='~').contains(&c) { c as u8 } else { b'?' })
}

/// The width of `text` in points at `size`.
pub fn width(text: &str, font: Font, size: f64) -> f64 {
    let units: u32 = ascii(text)
        .map(|byte| match font {
            Font::Regular => u32::from(HELVETICA[usize::from(byte - 32)]),
            Font::Bold => u32::from(HELVETICA_BOLD[usize::from(byte - 32)]),
        })
        .sum();
    f64::from(units) * size / 1_000.0
}

/// Words of `text` in lines no wider than `max`.
pub fn wrap(text: &str, font: Font, size: f64, max: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if !line.is_empty() && width(&candidate, font, size) > max {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// One page's drawing operations.
#[derive(Default)]
pub struct Page {
    ops: String,
}

impl Page {
    /// A filled rectangle whose top-left corner is at (`x`, `y`).
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: Rgb) {
        let _ = writeln!(self.ops, "{} rg {x:.2} {:.2} {w:.2} {h:.2} re f", fill.ops(), PAGE_HEIGHT - y - h);
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: Rgb, thickness: f64) {
        let _ = writeln!(
            self.ops,
            "{} RG {thickness:.2} w {x1:.2} {:.2} m {x2:.2} {:.2} l S",
            stroke.ops(),
            PAGE_HEIGHT - y1,
            PAGE_HEIGHT - y2
        );
    }

    /// Text whose baseline starts at (`x`, `y`).
    pub fn text(&mut self, x: f64, y: f64, font: Font, size: f64, color: Rgb, text: &str) {
        let mut escaped = String::new();
        for byte in ascii(text) {
            if matches!(byte, b'(' | b')' | b'\\') {
                escaped.push('\\');
            }
            escaped.push(char::from(byte));
        }
        let _ = writeln!(
            self.ops,
            "BT {} rg /{} {size:.2} Tf {x:.2} {:.2} Td ({escaped}) Tj ET",
            color.ops(),
            font.resource(),
            PAGE_HEIGHT - y
        );
    }

    /// Text that ends at `right`.
    pub fn text_right(&mut self, right: f64, y: f64, font: Font, size: f64, color: Rgb, text: &str) {
        self.text(right - width(text, font, size), y, font, size, color, text);
    }
}

/// The document: the pages in order, as PDF bytes.
pub fn document(title: &str, pages: &[Page]) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    let page_ids: Vec<usize> = (0..pages.len()).map(|at| 6 + at * 2).collect();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        page_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" "),
        pages.len()
    ));
    for base in ["Helvetica", "Helvetica-Bold"] {
        objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>"));
    }
    let title: String = ascii(title).map(char::from).filter(|c| !matches!(c, '(' | ')' | '\\')).collect();
    objects.push(format!("<< /Title ({title}) /Producer (df-hyperliquid-core bench-runner) >>"));
    for (at, page) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            page_ids[at] + 1
        ));
        objects.push(format!("<< /Length {} >>\nstream\n{}endstream", page.ops.len(), page.ops));
    }
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (at, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{object}\nendobj\n", at + 1);
    }
    let xref = out.len();
    let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(out, "{offset:010} 00000 n ");
    }
    let _ =
        write!(out, "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1);
    out.into_bytes()
}
