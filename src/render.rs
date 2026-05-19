use std::fmt::Write as _;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

pub fn flush(buf: &Buffer) {
    let area = buf.area();
    let cap = area.width as usize * area.height as usize * 4;
    let mut out = String::with_capacity(cap);

    out.push_str("\x1b[H");
    let mut last_style: Option<Style> = None;

    for y in 0..area.height {
        let _ = write!(out, "\x1b[{};1H", y + 1);
        for x in 0..area.width {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let style = cell.style();
            if last_style != Some(style) {
                out.push_str("\x1b[0m");
                emit_style(&mut out, style);
                last_style = Some(style);
            }
            out.push_str(cell.symbol());
        }
    }
    out.push_str("\x1b[0m");
    print!("{out}");
}

fn emit_style(out: &mut String, s: Style) {
    if let Some(fg) = s.fg {
        emit_color(out, fg, false);
    }
    if let Some(bg) = s.bg {
        emit_color(out, bg, true);
    }
    let m = s.add_modifier;
    if m.contains(Modifier::BOLD) {
        out.push_str("\x1b[1m");
    }
    if m.contains(Modifier::DIM) {
        out.push_str("\x1b[2m");
    }
    if m.contains(Modifier::ITALIC) {
        out.push_str("\x1b[3m");
    }
    if m.contains(Modifier::UNDERLINED) {
        out.push_str("\x1b[4m");
    }
    if m.contains(Modifier::REVERSED) {
        out.push_str("\x1b[7m");
    }
}

fn emit_color(out: &mut String, c: Color, bg: bool) {
    let (base, bright, prefix_256, prefix_rgb) = if bg {
        (40u8, 100u8, "\x1b[48;5;", "\x1b[48;2;")
    } else {
        (30u8, 90u8, "\x1b[38;5;", "\x1b[38;2;")
    };
    match c {
        Color::Reset => out.push_str(if bg { "\x1b[49m" } else { "\x1b[39m" }),
        Color::Black => {
            let _ = write!(out, "\x1b[{}m", base);
        }
        Color::Red => {
            let _ = write!(out, "\x1b[{}m", base + 1);
        }
        Color::Green => {
            let _ = write!(out, "\x1b[{}m", base + 2);
        }
        Color::Yellow => {
            let _ = write!(out, "\x1b[{}m", base + 3);
        }
        Color::Blue => {
            let _ = write!(out, "\x1b[{}m", base + 4);
        }
        Color::Magenta => {
            let _ = write!(out, "\x1b[{}m", base + 5);
        }
        Color::Cyan => {
            let _ = write!(out, "\x1b[{}m", base + 6);
        }
        Color::Gray => {
            let _ = write!(out, "\x1b[{}m", base + 7);
        }
        Color::DarkGray => {
            let _ = write!(out, "\x1b[{}m", bright);
        }
        Color::LightRed => {
            let _ = write!(out, "\x1b[{}m", bright + 1);
        }
        Color::LightGreen => {
            let _ = write!(out, "\x1b[{}m", bright + 2);
        }
        Color::LightYellow => {
            let _ = write!(out, "\x1b[{}m", bright + 3);
        }
        Color::LightBlue => {
            let _ = write!(out, "\x1b[{}m", bright + 4);
        }
        Color::LightMagenta => {
            let _ = write!(out, "\x1b[{}m", bright + 5);
        }
        Color::LightCyan => {
            let _ = write!(out, "\x1b[{}m", bright + 6);
        }
        Color::White => {
            let _ = write!(out, "\x1b[{}m", bright + 7);
        }
        Color::Indexed(i) => {
            let _ = write!(out, "{}{}m", prefix_256, i);
        }
        Color::Rgb(r, g, b) => {
            let _ = write!(out, "{}{};{};{}m", prefix_rgb, r, g, b);
        }
    }
}
