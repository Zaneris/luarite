use crate::metrics::MetricsCollector;

// Minimal 4x6 bitmap font for a limited ASCII subset (A-Z, 0-9, space, colon, period, slash, dash, underscore, pipe)
// Each glyph is 4x6 pixels, stored as 6 rows of 4 bits (LSB on the right).
fn glyph_bits(c: char) -> [u8; 6] {
    use std::collections::HashMap;
    fn map() -> HashMap<char, [u8; 6]> {
        let mut m = HashMap::new();
        // Digits 0-9
        m.insert('0', [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110]);
        m.insert('1', [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0111]);
        m.insert('2', [0b0110, 0b1001, 0b0001, 0b0010, 0b0100, 0b1111]);
        m.insert('3', [0b1110, 0b0001, 0b0110, 0b0001, 0b0001, 0b1110]);
        m.insert('4', [0b0001, 0b0011, 0b0101, 0b1001, 0b1111, 0b0001]);
        m.insert('5', [0b1111, 0b1000, 0b1110, 0b0001, 0b1001, 0b0110]);
        m.insert('6', [0b0110, 0b1000, 0b1110, 0b1001, 0b1001, 0b0110]);
        m.insert('7', [0b1111, 0b0001, 0b0010, 0b0010, 0b0100, 0b0100]);
        m.insert('8', [0b0110, 0b1001, 0b0110, 0b1001, 0b1001, 0b0110]);
        m.insert('9', [0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110]);
        // A-Z
        m.insert('A', [0b0110, 0b1001, 0b1001, 0b1111, 0b1001, 0b1001]);
        m.insert('B', [0b1110, 0b1001, 0b1110, 0b1001, 0b1001, 0b1110]);
        m.insert('C', [0b0110, 0b1001, 0b1000, 0b1000, 0b1001, 0b0110]);
        m.insert('D', [0b1110, 0b1001, 0b1001, 0b1001, 0b1001, 0b1110]);
        m.insert('E', [0b1111, 0b1000, 0b1110, 0b1000, 0b1000, 0b1111]);
        m.insert('F', [0b1111, 0b1000, 0b1110, 0b1000, 0b1000, 0b1000]);
        m.insert('G', [0b0110, 0b1001, 0b1000, 0b1011, 0b1001, 0b0110]);
        m.insert('H', [0b1001, 0b1001, 0b1111, 0b1001, 0b1001, 0b1001]);
        m.insert('I', [0b0111, 0b0010, 0b0010, 0b0010, 0b0010, 0b0111]);
        m.insert('J', [0b0001, 0b0001, 0b0001, 0b1001, 0b1001, 0b0110]);
        m.insert('K', [0b1001, 0b1010, 0b1100, 0b1010, 0b1010, 0b1001]);
        m.insert('L', [0b1000, 0b1000, 0b1000, 0b1000, 0b1000, 0b1111]);
        m.insert('M', [0b1001, 0b1111, 0b1111, 0b1001, 0b1001, 0b1001]);
        m.insert('N', [0b1001, 0b1101, 0b1101, 0b1011, 0b1011, 0b1001]);
        m.insert('O', [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110]);
        m.insert('P', [0b1110, 0b1001, 0b1110, 0b1000, 0b1000, 0b1000]);
        m.insert('Q', [0b0110, 0b1001, 0b1001, 0b1011, 0b1010, 0b0111]);
        m.insert('R', [0b1110, 0b1001, 0b1110, 0b1010, 0b1001, 0b1001]);
        m.insert('S', [0b0111, 0b1000, 0b0110, 0b0001, 0b0001, 0b1110]);
        m.insert('T', [0b1111, 0b0010, 0b0010, 0b0010, 0b0010, 0b0010]);
        m.insert('U', [0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110]);
        m.insert('V', [0b1001, 0b1001, 0b1001, 0b1001, 0b0110, 0b0110]);
        m.insert('W', [0b1001, 0b1001, 0b1001, 0b1111, 0b1111, 0b1001]);
        m.insert('X', [0b1001, 0b1001, 0b0110, 0b0110, 0b1001, 0b1001]);
        m.insert('Y', [0b1001, 0b1001, 0b0110, 0b0010, 0b0010, 0b0010]);
        m.insert('Z', [0b1111, 0b0001, 0b0010, 0b0100, 0b1000, 0b1111]);
        // Punct/space
        m.insert(' ', [0, 0, 0, 0, 0, 0]);
        m.insert(':', [0, 0b0010, 0, 0, 0b0010, 0]);
        m.insert('.', [0, 0, 0, 0, 0b0011, 0b0011]);
        m.insert('/', [0b0001, 0b0010, 0b0010, 0b0100, 0b0100, 0b1000]);
        m.insert('-', [0, 0, 0b1111, 0, 0, 0]);
        m.insert('_', [0, 0, 0, 0, 0, 0b1111]);
        m.insert('|', [0b0010, 0b0010, 0b0010, 0b0010, 0b0010, 0b0010]);
        m
    }
    thread_local! {
        static GLYPHS: std::cell::RefCell<Option<std::collections::HashMap<char, [u8;6]>>> = const { std::cell::RefCell::new(None) };
    }
    GLYPHS.with(|cell| {
        if cell.borrow().is_none() {
            *cell.borrow_mut() = Some(map());
        }
        let map = cell.borrow();
        if let Some(map) = map.as_ref() {
            *map.get(&c).unwrap_or(&[0; 6])
        } else {
            [0; 6]
        }
    })
}

fn draw_char(rgba: &mut [u8], tex_w: u32, x: u32, y: u32, c: char, color: [u8; 4]) {
    let bits = glyph_bits(c);
    for (row, mask) in bits.iter().enumerate() {
        for col in 0..4u32 {
            let on = (mask >> (3 - col)) & 1 == 1;
            if on {
                let px = x + col;
                let py = y + row as u32;
                let idx = ((py * tex_w + px) * 4) as usize;
                if idx + 3 < rgba.len() {
                    rgba[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

pub fn rasterize_hud(lines: &[String], metrics: &MetricsCollector) -> (Vec<u8>, u32, u32) {
    // Compose first line from metrics
    let stats = metrics.get_performance_stats();
    let fps = if metrics.current_metrics().cpu_frame_ms > 0.0 {
        1000.0 / metrics.current_metrics().cpu_frame_ms
    } else {
        0.0
    };
    let p99 = stats.get("cpu_frame_p99_ms").copied().unwrap_or(0.0);
    let sprites = metrics.current_metrics().sprites_submitted;
    let ffi = metrics.current_metrics().ffi_calls;
    let header = format!("FPS:{:.1} P99:{:.1} SPR:{} FFI:{}", fps, p99, sprites, ffi);

    // Prepare text buffer (max 10 lines, 64 chars each)
    let mut all_lines: Vec<String> = Vec::new();
    all_lines.push(header);
    for s in lines.iter().rev().take(9) {
        all_lines.push(s.clone());
    }
    let max_chars = 64usize;
    let char_w = 5u32; // 4px glyph + 1px spacing
    let char_h = 7u32; // 6px glyph + 1px spacing
    let width = (max_chars as u32) * char_w;
    let height = (all_lines.len() as u32) * char_h;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    // Background translucent black
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            rgba[idx] = 0;
            rgba[idx + 1] = 0;
            rgba[idx + 2] = 0;
            rgba[idx + 3] = 160;
        }
    }

    // Draw lines (uppercase, unsupported chars → space)
    for (li, line) in all_lines.iter().enumerate() {
        let mut s = line.to_uppercase();
        if s.len() > max_chars {
            s.truncate(max_chars);
        }
        for (ci, ch) in s.chars().enumerate() {
            let cx = (ci as u32) * char_w + 1;
            let cy = (li as u32) * char_h + 1;
            draw_char(&mut rgba, width, cx, cy, ch, [255, 255, 255, 255]);
        }
    }

    (rgba, width, height)
}
