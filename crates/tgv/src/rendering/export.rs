// Author: Samuel Ahuno
// Date: 2026-02-23
// Purpose: Export the current terminal buffer to HTML, SVG, or plain-text files.

use ratatui::{buffer::Buffer, style::Color};

// ── Colour helpers ────────────────────────────────────────────────────────────

/// Convert a ratatui `Color` to a CSS colour string.
fn color_to_css(color: Color) -> &'static str {
    // We box the computed string into a leak-free static via a small match on
    // the common cases; the RGB arm uses a helper that returns an owned String.
    match color {
        Color::Reset => "inherit",
        Color::Black => "#000000",
        Color::Red => "#800000",
        Color::Green => "#008000",
        Color::Yellow => "#808000",
        Color::Blue => "#000080",
        Color::Magenta => "#800080",
        Color::Cyan => "#008080",
        Color::Gray => "#c0c0c0",
        Color::DarkGray => "#808080",
        Color::LightRed => "#ff0000",
        Color::LightGreen => "#00ff00",
        Color::LightYellow => "#ffff00",
        Color::LightBlue => "#0000ff",
        Color::LightMagenta => "#ff00ff",
        Color::LightCyan => "#00ffff",
        Color::White => "#ffffff",
        // Indexed and Rgb are handled in the owned-string path below.
        _ => "inherit",
    }
}

/// Return an owned CSS colour string (handles Rgb and Indexed cases).
fn color_to_css_owned(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Indexed(i) => {
            // Map the 256-colour palette index to an RGB approximation.
            let (r, g, b) = indexed_to_rgb(i);
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        }
        other => color_to_css(other).to_string(),
    }
}

/// Approximate 256-colour ANSI index → (r, g, b).
fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        // Standard colours 0-15 use the named-colour approximations.
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        // 216-colour cube: indices 16-231
        16..=231 => {
            let n = idx - 16;
            let b = n % 6;
            let g = (n / 6) % 6;
            let r = n / 36;
            let scale = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (scale(r), scale(g), scale(b))
        }
        // Greyscale ramp: indices 232-255
        232..=255 => {
            let v = 8 + (idx - 232) * 10;
            (v, v, v)
        }
    }
}

/// Append an HTML/XML-safe representation of `c` to `buf`.
fn push_html_escaped(buf: &mut String, c: char) {
    match c {
        '&' => buf.push_str("&amp;"),
        '<' => buf.push_str("&lt;"),
        '>' => buf.push_str("&gt;"),
        '"' => buf.push_str("&quot;"),
        ' ' => buf.push_str("&nbsp;"),
        c => buf.push(c),
    }
}

// ── Plain text ────────────────────────────────────────────────────────────────

/// Render the buffer as plain text (characters only, no colour).
pub fn buffer_to_text(buf: &Buffer) -> String {
    let mut out = String::with_capacity(
        ((buf.area.width as usize) + 1) * (buf.area.height as usize),
    );
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = buf.cell((x, y)).map(|c| c.symbol().to_string()).unwrap_or_else(|| " ".to_string());
            out.push_str(&cell);
        }
        out.push('\n');
    }
    out
}

// ── HTML export ───────────────────────────────────────────────────────────────

/// Render the buffer as a self-contained HTML file with inline CSS colours.
pub fn buffer_to_html(buf: &Buffer) -> String {
    let mut body = String::new();

    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(cell) = buf.cell((x, y)) {
                let symbol = cell.symbol();
                let fg = color_to_css_owned(cell.fg);
                let bg = color_to_css_owned(cell.bg);

                body.push_str("<span style=\"color:");
                body.push_str(&fg);
                body.push_str(";background-color:");
                body.push_str(&bg);
                body.push_str("\">");
                for ch in symbol.chars() {
                    push_html_escaped(&mut body, ch);
                }
                body.push_str("</span>");
            }
        }
        body.push('\n');
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>TGV snapshot</title>
  <style>
    body {{
      background: #1e1e1e;
      margin: 0;
      padding: 1em;
    }}
    pre {{
      font-family: "JetBrains Mono", "Fira Code", "Cascadia Code",
                   "DejaVu Sans Mono", "Courier New", monospace;
      font-size: 13px;
      line-height: 1.4;
      white-space: pre;
      margin: 0;
    }}
  </style>
</head>
<body>
<pre>{body}</pre>
</body>
</html>
"#
    )
}

// ── SVG export ────────────────────────────────────────────────────────────────

/// Pixels per character cell.
const CHAR_W: u32 = 8;
const CHAR_H: u32 = 16;

/// Render the buffer as an SVG file.
///
/// Each cell becomes a `<rect>` (background) plus a `<text>` (character).
/// The SVG is fully self-contained — no external fonts or scripts.
pub fn buffer_to_svg(buf: &Buffer) -> String {
    let width = buf.area.width as u32 * CHAR_W;
    let height = buf.area.height as u32 * CHAR_H;

    let mut rects = String::new();
    let mut texts = String::new();

    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let px = x as u32 * CHAR_W;
            let py = y as u32 * CHAR_H;
            let bg = color_to_css_owned(cell.bg);

            // Background rectangle (skip for "inherit"/transparent backgrounds).
            if bg != "inherit" {
                rects.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{py}\" width=\"{CHAR_W}\" height=\"{CHAR_H}\" fill=\"{bg}\"/>\n"
                ));
            }

            let symbol = cell.symbol();
            // Skip blank / space characters — no <text> needed.
            let is_blank = symbol.chars().all(|c| c == ' ' || c == '\u{0}');
            if is_blank {
                continue;
            }

            let fg = color_to_css_owned(cell.fg);
            // Text baseline sits at the bottom of the cell.
            let text_y = py + CHAR_H - 3;

            // SVG-escape the symbol.
            let mut escaped = String::new();
            for ch in symbol.chars() {
                match ch {
                    '&' => escaped.push_str("&amp;"),
                    '<' => escaped.push_str("&lt;"),
                    '>' => escaped.push_str("&gt;"),
                    '"' => escaped.push_str("&quot;"),
                    '\'' => escaped.push_str("&apos;"),
                    c => escaped.push(c),
                }
            }

            texts.push_str(&format!(
                "<text x=\"{px}\" y=\"{text_y}\" fill=\"{fg}\">{escaped}</text>\n"
            ));
        }
    }

    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     width="{width}" height="{height}"
     viewBox="0 0 {width} {height}">
  <defs>
    <style>
      text {{
        font-family: "JetBrains Mono", "Fira Code", "Cascadia Code",
                     "DejaVu Sans Mono", "Courier New", monospace;
        font-size: {CHAR_H}px;
        font-weight: normal;
      }}
    </style>
  </defs>
  <!-- background fill -->
  <rect width="{width}" height="{height}" fill="#1e1e1e"/>
  <!-- cell backgrounds -->
{rects}
  <!-- characters -->
{texts}
</svg>
"##
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, style::Style};

    fn make_buf(content: &str, w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect { x: 0, y: 0, width: w, height: h });
        buf.set_string(0, 0, content, Style::default());
        buf
    }

    #[test]
    fn test_buffer_to_text_contains_content() {
        let buf = make_buf("Hello", 10, 3);
        let text = buffer_to_text(&buf);
        assert!(text.contains("Hello"));
        // should have 3 lines
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn test_buffer_to_html_structure() {
        let buf = make_buf("Hi", 5, 2);
        let html = buffer_to_html(&buf);
        assert!(html.contains("<!DOCTYPE html>"), "missing doctype");
        assert!(html.contains("<pre>"), "missing <pre>");
        assert!(html.contains("<span style="), "missing spans");
        // Each character renders in its own <span> so check individually.
        assert!(html.contains(">H<"), "missing 'H' in span");
        assert!(html.contains(">i<"), "missing 'i' in span");
    }

    #[test]
    fn test_buffer_to_html_escapes_angle_brackets() {
        use ratatui::style::Style;
        let mut buf = Buffer::empty(Rect { x: 0, y: 0, width: 10, height: 1 });
        buf.set_string(0, 0, "<tag>", Style::default());
        let html = buffer_to_html(&buf);
        // Each char is escaped individually inside its own span.
        assert!(html.contains("&lt;"), "< not escaped");
        assert!(html.contains("&gt;"), "> not escaped");
        assert!(!html.contains("<tag>"), "<tag> should not appear unescaped");
    }

    #[test]
    fn test_buffer_to_svg_structure() {
        let buf = make_buf("TGV", 10, 3);
        let svg = buffer_to_svg(&buf);
        assert!(svg.contains("<?xml"), "missing xml declaration");
        assert!(svg.contains("<svg"), "missing <svg>");
        assert!(svg.contains("<rect"), "missing <rect>");
        assert!(svg.contains("<text"), "missing <text>");
        // Each character is in a separate <text> element.
        assert!(svg.contains(">T<"), "missing 'T' in text element");
        assert!(svg.contains(">G<"), "missing 'G' in text element");
        assert!(svg.contains(">V<"), "missing 'V' in text element");
    }

    #[test]
    fn test_buffer_to_svg_dimensions() {
        let buf = make_buf("X", 20, 5);
        let svg = buffer_to_svg(&buf);
        let expected_w = 20u32 * CHAR_W;
        let expected_h = 5u32 * CHAR_H;
        assert!(svg.contains(&format!("width=\"{expected_w}\"")));
        assert!(svg.contains(&format!("height=\"{expected_h}\"")));
    }

    #[test]
    fn test_color_to_css_rgb() {
        assert_eq!(color_to_css_owned(Color::Rgb(255, 128, 0)), "#ff8000");
        assert_eq!(color_to_css_owned(Color::Rgb(0, 0, 0)), "#000000");
    }

    #[test]
    fn test_color_to_css_named() {
        assert_eq!(color_to_css_owned(Color::White), "#ffffff");
        assert_eq!(color_to_css_owned(Color::Reset), "inherit");
    }
}
