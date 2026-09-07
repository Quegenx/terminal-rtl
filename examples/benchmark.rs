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
        "בדיקת תצוגה! English 456 src/terminal/input.rs",
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
    // Compare history browsing with the old whole-screen clone and a viewport
    // read. Links exercise shared metadata as well as Hebrew reordering.
    let mut parser = vt100::Parser::new_with_callbacks(
        24,
        100,
        10000,
        terminal_rtl::protocol::Protocol::default(),
    );
    parser.process(b"\x1b]8;;https://example.com/benchmark\x07");
    for row in 0..10000 {
        parser.process(format!("History {row:05}: שלום עולם\r\n").as_bytes());
    }
    for clone_history in [true, false] {
        let mut renderer = Renderer::default();
        let start = Instant::now();
        for frame in 0..frames.min(200) {
            let offset = 5000 + frame % 100;
            let mut output = Vec::new();
            if clone_history {
                let mut view = parser.screen().clone();
                view.set_scrollback(offset);
                renderer
                    .render(&view, true, Direction::Auto, &mut output)
                    .unwrap();
            } else {
                renderer
                    .render_scrollback(parser.screen(), offset, true, Direction::Auto, &mut output)
                    .unwrap();
            }
            black_box(output);
        }
        println!(
            "10,000 linked history rows (clone={clone_history}): {:.3} ms/browse frame",
            start.elapsed().as_secs_f64() * 1000.0 / frames.min(200) as f64
        );
    }
}
