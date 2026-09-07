use terminal_rtl::{
    display::{Direction, Renderer},
    protocol::Protocol,
};

#[test]
fn historical_rows_survive_width_changes() {
    let mut p = vt100::Parser::new(2, 8, 10);
    p.process(b"abcdefgh\r\nijklmnop\r\nnext");
    for width in [12, 5, 1, 8] {
        p.screen_mut().set_size(2, width);
        p.screen_mut().set_scrollback(1);
        Renderer::default()
            .render(p.screen(), true, Direction::Auto, &mut Vec::new())
            .unwrap();
    }
    assert_eq!(p.screen().history_since(0).next().unwrap().len(), 8);
}

#[test]
fn minimum_grids_and_wide_cell_resize_remain_valid() {
    let mut p = vt100::Parser::new(2, 4, 10);
    p.process("ab界".as_bytes());
    p.screen_mut().set_size(2, 3);
    p.process(b"\x1b[1;3H\x1b[X");
    assert!(!p.screen().cell(0, 2).unwrap().is_wide());
    for (rows, cols) in [(1, 1), (1, 2), (0, 0)] {
        let mut p = vt100::Parser::new(rows, cols, 10);
        for text in ["界界abc", "\x1b[H\x1b[@界", "\x1b[P\x1b[X\x1b[K"] {
            p.process(text.as_bytes());
        }
        assert!(p.screen().size().0 >= 1);
    }
}

#[test]
fn complete_semicolon_links_and_oversized_osc_recovery() {
    for separators in [12, 13, 14, 16, 100] {
        let uri = format!("https://example.com/{}end", "a;".repeat(separators));
        let mut p = vt100::Parser::new_with_callbacks(2, 20, 0, Protocol::default());
        for byte in format!("\x1b]8;;{uri}\x1b\\X").bytes() {
            p.process(&[byte]);
        }
        assert_eq!(
            p.screen().cell(0, 0).unwrap().hyperlink().unwrap().uri(),
            uri
        );
        p.process(b"\x1b]8;;");
        for _ in 0..2048 {
            p.process(&[b'a'; 4096]);
        }
        p.process(b"\x1b");
        p.process(b"\\Y");
        assert_eq!(p.screen().contents(), "XY");
        assert!(p.screen().cell(0, 1).unwrap().hyperlink().is_none());
    }
}

#[test]
fn reset_wrap_and_origin_modes() {
    let mut p = vt100::Parser::new_with_callbacks(4, 4, 10, Protocol::default());
    p.process(b"\x1b[?7labcdef");
    assert_eq!(p.screen().contents(), "abcf");
    p.process(b"\x1b[2;4r\x1b[?6h\x1b[H\x1b[6n");
    assert_eq!(p.callbacks().replies, b"\x1b[1;1R");
    p.process(b"\x1b[?1004h\x1b[?2026h\x07\x1bc");
    assert!(!p.callbacks().focus_events);
    assert!(!p.callbacks().synchronized_output);
    assert!(!p.callbacks().bell);
    assert!(p.callbacks().replies.is_empty());
    assert_eq!(p.screen().reset_generation(), 1);
    p.process(b"abcde");
    assert_eq!(p.screen().cell(1, 0).unwrap().contents(), "e");
}

#[test]
fn synchronous_history_delivery_survives_bursts_zero_retention_and_resets() {
    for retention in [0, 1, 2] {
        let mut p = vt100::Parser::new(2, 12, retention);
        let mut delivered = Vec::new();
        let bytes = b"A\r\nB\r\nC\r\nD\x1b[2S\x1bcE\r\nF\r\nG\x1b[3J\r\nH";
        p.process_with_history(bytes, |rows| {
            for row in rows {
                delivered.push(row.iter().map(|c| c.contents()).collect::<String>());
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        assert_eq!(delivered, ["A", "B", "C", "D", "E", "F"]);
        assert!(p.screen().history_since(0).count() <= retention);
    }
    // RIS may produce fewer, equal, or more rows before the next render.
    // Delivery must not compare counters from two different screens.
    for after_reset in [1, 2, 5] {
        let mut p = vt100::Parser::new(1, 12, 1);
        let mut delivered = Vec::new();
        let mut bytes = String::from("old0\r\nold1\r\n\x1bc");
        for row in 0..after_reset {
            bytes.push_str(&format!("new{row}\r\n"));
        }
        p.process_with_history(bytes.as_bytes(), |rows| {
            for row in rows {
                delivered.push(row.iter().map(|c| c.contents()).collect::<String>());
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        let expected: Vec<_> = ["old0".into(), "old1".into()]
            .into_iter()
            .chain((0..after_reset).map(|row| format!("new{row}")))
            .collect();
        assert_eq!(delivered, expected);
    }
}

#[test]
fn prose_layout_preserves_exact_space_runs_and_columns_remain_opt_in_default() {
    use terminal_rtl::display::{Layout, visual_row_with_layout};
    for (logical, prose, columns) in [
        ("שלום עולם", "םלוע םולש", "םלוע םולש"),
        ("שלום  עולם", "םלוע  םולש", "םולש  םלוע"),
        ("שלום   עולם", "םלוע   םולש", "םולש   םלוע"),
        ("  שלום  עולם", "  םלוע  םולש", "  םולש  םלוע"),
    ] {
        let mut p = vt100::Parser::new(1, 30, 0);
        p.process(logical.as_bytes());
        let rows = p.screen().viewport_rows();
        for (layout, expected) in [(Layout::Prose, prose), (Layout::Columns, columns)] {
            let visual = visual_row_with_layout(&rows[0], true, Direction::Auto, layout);
            let actual: String = visual.glyphs.iter().map(|g| g.text.as_str()).collect();
            assert_eq!(actual.trim_end(), expected, "{logical:?} {layout:?}");
            for glyph in &visual.glyphs {
                assert!(glyph.width <= 2);
            }
        }
    }
}

#[test]
fn exit_replay_preserves_links_after_resize_without_markdown_decoration() {
    use terminal_rtl::display::{Layout, replay};
    let mut p = vt100::Parser::new_with_callbacks(2, 8, 10, Protocol::default());
    let uri = format!("https://example.com/{}end", "part;".repeat(20));
    p.process(format!("\x1b]8;;{uri}\x07ABCDEFGH\r\n12345678\r\nEND\x1b]8;;\x07").as_bytes());
    p.screen_mut().set_size(2, 12);
    let mut host = vt100::Parser::new_with_callbacks(8, 20, 10, Protocol::default());
    host.process(&replay(p.screen(), true, Direction::Auto, Layout::Columns));
    assert!(host.screen().contents().contains("ABCDEFGH"));
    assert_eq!(
        host.screen().cell(0, 0).unwrap().hyperlink().unwrap().uri(),
        uri
    );
}

#[test]
fn wide_cells_remain_paired_through_editing_and_resizes() {
    let operations = [
        "界", "a", "\x1b[H", "\x1b[@", "\x1b[P", "\x1b[X", "\x1b[K", "\r\n", "\x1b[2C", "\x1b[2D",
        "\x1b[?7l", "\x1b[?7h",
    ];
    let mut p = vt100::Parser::new(2, 4, 4);
    let mut seed = 42u32;
    for step in 0..2000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        if step % 7 == 0 {
            p.screen_mut()
                .set_size(1 + (seed % 3) as u16, 1 + ((seed >> 8) % 6) as u16);
        }
        p.process(operations[(seed as usize >> 16) % operations.len()].as_bytes());
        let (rows, cols) = p.screen().size();
        for row in 0..rows {
            for col in 0..cols {
                let cell = p.screen().cell(row, col).unwrap();
                if cell.is_wide() {
                    assert!(
                        col + 1 < cols
                            && p.screen()
                                .cell(row, col + 1)
                                .unwrap()
                                .is_wide_continuation(),
                        "step {step}"
                    );
                }
                if cell.is_wide_continuation() {
                    assert!(
                        col > 0 && p.screen().cell(row, col - 1).unwrap().is_wide(),
                        "step {step}"
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_osc_is_rejected_without_creating_a_truncated_link() {
    for malformed in [
        b"https://example.com/\x01bad".as_slice(),
        b"https://example.com/\x1b[2Jbad",
    ] {
        let mut p = vt100::Parser::new_with_callbacks(2, 20, 0, Protocol::default());
        p.process(b"\x1b]8;;");
        p.process(malformed);
        p.process(b"\x1b\\OK");
        assert_eq!(p.screen().contents(), "OK");
        assert!(p.screen().cell(0, 0).unwrap().hyperlink().is_none());
    }
}

#[test]
fn independent_host_fixtures() {
    let Some(directory) = std::env::var_os("RTL_TEST_AUDIT_CAPTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    let mut p = vt100::Parser::new_with_callbacks(4, 8, 10, Protocol::default());
    let source =
        b"\x1b[?7labcdefghijk\x1b[2;4r\x1b[?6h\x1b[H\x1b[6n\x1b[?1004h\x1b[?2026h\x1bc\x1b[6n";
    p.process(source);
    // Query replies before RIS are intentionally discarded as stale in Protocol.
    std::fs::write(directory.join("modes-input"), source).unwrap();
    std::fs::write(directory.join("modes-replies"), &p.callbacks().replies).unwrap();
    let mut p = vt100::Parser::new_with_callbacks(2, 8, 10, Protocol::default());
    let uri = format!("https://example.com/{}end", "p;".repeat(30));
    p.process(format!("\x1b]8;;{uri}\x1b\\ABCDEFGH\r\n12345678\r\nEND\x1b]8;;\x1b\\").as_bytes());
    p.screen_mut().set_size(2, 12);
    std::fs::write(
        directory.join("replay"),
        terminal_rtl::display::replay(
            p.screen(),
            true,
            Direction::Auto,
            terminal_rtl::display::Layout::Columns,
        ),
    )
    .unwrap();
    let mut p = vt100::Parser::new(2, 4, 10);
    p.process("ab界\r\nnext\r\nend".as_bytes());
    p.screen_mut().set_size(2, 3);
    p.screen_mut().set_scrollback(1);
    let mut bytes = b"\x1b[?7l".to_vec();
    Renderer::default()
        .render(p.screen(), true, Direction::Auto, &mut bytes)
        .unwrap();
    std::fs::write(directory.join("geometry"), bytes).unwrap();
}

#[test]
fn prose_mixed_text_columns_links_and_coordinates_stay_consistent() {
    use terminal_rtl::display::{Layout, visual_row_with_layout};
    for (logical, expected) in [
        ("שלום  English  עולם", "םלוע  English  םולש"),
        ("שלום  (123)  עולם!", "!םלוע  (123)  םולש"),
        ("│ שלום  עולם │ English │", "│ םלוע  םולש │ English │"),
    ] {
        let mut p = vt100::Parser::new_with_callbacks(1, 40, 0, Protocol::default());
        p.process(b"\x1b]8;;https://example.com/full;destination\x07");
        p.process(logical.as_bytes());
        let cells = p.screen().viewport_rows().remove(0);
        let visual = visual_row_with_layout(&cells, true, Direction::Auto, Layout::Prose);
        let actual: String = visual.glyphs.iter().map(|g| g.text.as_str()).collect();
        assert_eq!(actual.trim_end(), expected);
        for (logical, cell) in cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.has_contents())
        {
            let mapped = usize::from(visual.logical_to_visual[logical]);
            assert_eq!(usize::from(visual.visual_to_logical[mapped]), logical);
            assert_eq!(visual.glyphs[mapped].hyperlink.as_ref(), cell.hyperlink());
        }
    }
    let mut p = vt100::Parser::new(1, 20, 0);
    p.process("שלום English".as_bytes());
    let cells = p.screen().viewport_rows().remove(0);
    let text = |direction, enabled| {
        visual_row_with_layout(&cells, enabled, direction, Layout::Prose)
            .glyphs
            .iter()
            .map(|g| g.text.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    };
    assert_eq!(text(Direction::Ltr, true), "םולש English");
    assert_eq!(text(Direction::Rtl, true), "English םולש");
    assert_eq!(text(Direction::Rtl, false), "שלום English");
}
