//! CPU-only measurement; terminal I/O and model latency are intentionally excluded.
use std::{hint::black_box, time::Instant};
use terminal_rtl::display::{Direction, Renderer};

fn main() {
    let frames: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2000".into())
        .parse()
        .expect("positive frame count");
    assert!(frames > 0);
    let mut parser = vt100::Parser::new(24, 100, 0);
    let mut renderer = Renderer::default();
    let messages = [
        "שלום עולם! English 123 src/main.rs",
        "בדיקת תצוגה! English 456 src/input.rs",
    ];
    let start = Instant::now();
    let mut bytes = 0;
    for frame in 0..frames {
        for row in 0..24 {
            parser.process(
                format!("\x1b[{};1H\x1b[2K│ {}  │", row + 1, messages[frame % 2]).as_bytes(),
            );
        }
        let mut output = Vec::new();
        renderer
            .render(parser.screen(), true, Direction::Auto, &mut output)
            .unwrap();
        bytes += black_box(output.len());
    }
    let elapsed = start.elapsed();
    println!(
        "{frames} full 24x100 updates: {:.3} ms/frame; {bytes} output bytes (CPU only)",
        elapsed.as_secs_f64() * 1000.0 / frames as f64
    );
}
