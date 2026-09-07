use super::harness::PtyHarness;
use portable_pty::PtySize;
use std::{thread, time::Duration};

#[test]
fn retained_history_is_replayed_after_exit() {
    let mut harness = PtyHarness::new("history");
    assert_eq!(harness.finish(), 0);
    let output = String::from_utf8_lossy(&harness.output);
    let replay = output.rsplit("\x1b[?1049l").next().unwrap();
    assert!(replay.contains("HISTORY_000"), "{replay:?}");
    assert!(replay.contains("HISTORY_049"));
}

#[test]
fn real_pty_wheel_browses_history_without_changing_draft_and_preserves_native_mouse() {
    let mut harness = PtyHarness::new("wheel");
    harness.until("SCROLL_READY draft");
    assert!(String::from_utf8_lossy(&harness.output).contains("\x1b[?1006h"));
    harness.output.clear();
    harness.send(b"\x1b[<64;4;5M".repeat(10).as_slice());
    harness.until("WHEEL_HISTORY_010");
    harness.output.clear();
    harness.send(b"\x1b[<65;4;5M".repeat(10).as_slice());
    harness.until("SCROLL_READY draft");
    harness.send(b"x");
    harness.until("NATIVE_MOUSE_READY");
    harness.send(b"\x1b[<65;4;5M");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("WHEEL_OK"));
}

#[test]
fn inline_resume_uses_native_history_and_leaves_selection_to_the_terminal() {
    let mut harness = PtyHarness::new("inline-history");
    harness.until("INLINE_DRAFT_READY");
    // Allow the independent xterm.js check to consume only this synthetic fixture.
    if let Some(path) = std::env::var_os("RTL_TEST_HOST_CAPTURE") {
        std::fs::write(path, &harness.output).unwrap();
    }
    let output = String::from_utf8_lossy(&harness.output);
    assert!(
        !output.contains("\x1b[?1049h"),
        "native scrolling requires the normal buffer"
    );
    assert!(
        !output.contains("\x1b[?1006h"),
        "selection must not be captured when the child does not request mouse events"
    );
    let mut host = vt100::Parser::new_with_callbacks(
        12,
        60,
        1000,
        terminal_rtl::protocol::Protocol::default(),
    );
    host.process(b"EARLIER_SHELL_OUTPUT");
    host.process(&harness.output);
    assert!(!host.screen().alternate_screen());
    let history: Vec<_> = host.screen().history_since(0).collect();
    let text: String = history
        .iter()
        .flat_map(|row| row.iter().map(|cell| cell.contents()))
        .collect();
    assert!(text.contains("EARLIER_SHELL_OUTPUT"));
    assert!(text.contains("RESUMED_000"));
    assert!(text.contains("RESUMED_040"));
    let link = history
        .iter()
        .flat_map(|row| row.iter())
        .find_map(|cell| cell.hyperlink())
        .unwrap();
    assert_eq!(link.uri(), "https://example.com/complete/destination");
    assert!(host.screen().contents().contains("INLINE_DRAFT_READY"));
    assert!(host.screen().cell(9, 0).unwrap().hyperlink().is_none());
    assert!(host.screen().contents().contains("Powered by: Gal Havkin"));
    harness.send(b"x");
    assert_eq!(harness.finish(), 0);
}

#[test]
fn real_pty_minimum_geometry_browsing_and_replay_options() {
    for scenario in ["resize-replay", "resize-no-replay"] {
        let mut harness = PtyHarness::new(scenario);
        harness.until("GEOMETRY_READY");
        for (rows, cols) in [(3, 4), (1, 1), (14, 78)] {
            harness
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .unwrap();
            harness.host.screen_mut().set_size(rows, cols);
            harness.send(b"\x1b[5;2~");
            thread::sleep(Duration::from_millis(220));
            harness.send(b"x");
            thread::sleep(Duration::from_millis(220));
        }
        assert_eq!(harness.finish(), 0);
        let output = String::from_utf8_lossy(&harness.output);
        let replay = output.rsplit("\x1b[?1049l").next().unwrap();
        assert_eq!(replay.contains("GEOMETRY_000"), scenario == "resize-replay");
    }
}

#[test]
fn inline_bursts_and_child_clears_preserve_host_history() {
    let mut harness = PtyHarness::new("inline-burst");
    harness.until("BURST_READY");
    if let Some(path) = std::env::var_os("RTL_TEST_BURST_CAPTURE") {
        std::fs::write(path, &harness.output).unwrap();
    }
    let mut host = vt100::Parser::new(12, 60, 1000);
    host.process(b"EARLIER_SHELL_OUTPUT");
    host.process(&harness.output);
    let history: String = host
        .screen()
        .history_since(0)
        .flat_map(|row| row.into_iter().map(|c| c.contents().to_owned()))
        .collect();
    assert!(history.contains("EARLIER_SHELL_OUTPUT"));
    for line in 0..90 {
        assert!(
            history.contains(&format!("BURST_{line:03}")),
            "missing {line}"
        );
    }
    harness.send(b"x");
    assert_eq!(harness.finish(), 0);
}
