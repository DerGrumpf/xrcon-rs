//! Converts Xonotic color codes (`^0`..`^9`, `^xRGB`) to ANSI escapes.

/// ANSI sequence for each color code `^0`..`^9`. Every sequence starts
/// with a reset so codes do not combine with earlier ones.
const CODES: [&[u8]; 10] = [
    b"\x1b[0m",
    b"\x1b[0;1;31m",
    b"\x1b[0;1;32m",
    b"\x1b[0;1;33m",
    b"\x1b[0;1;34m",
    b"\x1b[0;1;36m",
    b"\x1b[0;1;35m",
    b"\x1b[0m",
    b"\x1b[0;1m",
    b"\x1b[0;1m",
];

/// Resets all attributes.
const RESET: &[u8] = b"\x1b[0m";

/// Value of one hex digit.
fn hex(c: u8) -> Option<i32> {
    char::from(c)
        .to_digit(16)
        .and_then(|v| i32::try_from(v).ok())
}

/// Reduces a `^xRGB` color (each channel 0..=15) to the nearest code 0..=9.
fn simplify(r: i32, g: i32, b: i32) -> usize {
    let (r, g, b) = (r * 17, g * 17, b * 17);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = f64::from(max - min);
    let hue = if min == max {
        0.0
    } else if r == max {
        60.0 * f64::from(g - b) / delta
    } else if g == max {
        60.0 * f64::from(b - r) / delta + 120.0
    } else {
        60.0 * f64::from(r - g) / delta + 240.0
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let sat = if max == 0 {
        0.0
    } else {
        1.0 - f64::from(min) / f64::from(max)
    };
    let val = f64::from(max) / 255.0;
    if sat < 0.2 && val < 0.5 {
        0
    } else if sat < 0.2 {
        7
    } else if hue < 36.0 {
        1
    } else if hue < 80.0 {
        3
    } else if hue < 150.0 {
        2
    } else if hue < 200.0 {
        5
    } else if hue < 270.0 {
        4
    } else if hue < 330.0 {
        6
    } else {
        1
    }
}

/// Parses the text after a `^`. Returns the color code (0..=9) and how
/// many bytes after the `^` it used.
fn parse_code(rest: &[u8]) -> Option<(usize, usize)> {
    match rest {
        [d @ b'0'..=b'9', ..] => Some((usize::from(*d - b'0'), 1)),
        [b'x', r, g, b, ..] => Some((simplify(hex(*r)?, hex(*g)?, hex(*b)?), 4)),
        _ => None,
    }
}

/// Replaces Xonotic color codes in `input` with ANSI escapes.
///
/// `^^` becomes a literal `^`. A `^` not followed by a valid code is kept.
/// Colors are reset at the end of every line.
pub fn colorize(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut colored = false;
    let mut i = 0;
    while let Some(&b) = input.get(i) {
        if b == b'^' {
            if input.get(i + 1) == Some(&b'^') {
                out.push(b'^');
                i += 2;
            } else if let Some((code, len)) = input.get(i + 1..).and_then(parse_code) {
                if let Some(seq) = CODES.get(code) {
                    out.extend_from_slice(seq);
                }
                colored = code != 0 && code != 7;
                i += 1 + len;
            } else {
                out.push(b'^');
                i += 1;
            }
        } else if b == b'\n' {
            if colored {
                out.extend_from_slice(RESET);
                colored = false;
            }
            out.push(b'\n');
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    if colored {
        out.extend_from_slice(RESET);
    }
    out
}
