use terminal_rtl::display::{Direction, Renderer, VisualRow, visual_row};

fn row(text: &str, width: u16) -> VisualRow {
    let mut parser = vt100::Parser::new(3, width, 0);
    parser.process(text.as_bytes());
    let cells: Vec<_> = (0..width)
        .map(|c| parser.screen().cell(0, c).unwrap().clone())
        .collect();
    visual_row(&cells, true, Direction::Auto)
}

fn text(row: &VisualRow) -> String {
    row.glyphs
        .iter()
        .map(|g| g.text.as_str())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn hebrew_and_punctuation_are_in_visual_order() {
    assert_eq!(text(&row("שלום עולם!", 40)), "!םלוע םולש");
}

#[test]
fn hyperlink_destinations_survive_rtl_rendering_and_redraw() {
    use terminal_rtl::protocol::Protocol;
    let mut parser = vt100::Parser::new_with_callbacks(3, 40, 10, Protocol::default());
    for byte in
        "\x1b]8;id=docs;https://example.com/a;b?lang=he\x1b\\שלום\x1b]8;;\x1b\\ plain".as_bytes()
    {
        parser.process(&[*byte]);
    }
    let mut renderer = Renderer::default();
    let mut output = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut output)
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains(";https://example.com/a;b?lang=he\x1b\\"),
        "{output:?}"
    );
    assert!(
        output.contains("םולש\x1b]8;;\x1b\\"),
        "close the link after its visual label: {output:?}"
    );
    // A changed destination with identical visible text still needs a repaint.
    parser.process("\x1b[H\x1b]8;;https://example.org/new\x1b\\שלום\x1b]8;;\x1b\\".as_bytes());
    let mut changed = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut changed)
        .unwrap();
    assert!(
        String::from_utf8(changed)
            .unwrap()
            .contains("https://example.org/new")
    );
}

#[test]
fn english_numbers_paths_and_cli_flags_keep_their_internal_order() {
    let output = text(&row("שלום English 123 src/main.rs --help", 60));
    assert!(
        output.contains("English 123 src/main.rs --help"),
        "{output}"
    );
    assert!(output.contains("םולש"));
    assert_eq!(text(&row("cargo test --locked", 40)), "cargo test --locked");
}

#[test]
fn mirrors_parentheses_and_keeps_marks_attached() {
    assert_eq!(text(&row("שלום (עולם)", 40)), "(םלוע) םולש");
    let output = text(&row("שָׁלוֹם", 40));
    assert_eq!(output, "םוֹלשָׁ");
}

#[test]
fn preserves_borders_indentation_and_prompt_columns() {
    assert_eq!(text(&row("│ שלום  │  ready │", 40)), "│ םולש  │  ready │");
    assert_eq!(text(&row("  > שלום עולם", 40)), "  > םלוע םולש");
}

#[test]
fn wide_cells_move_as_units_and_mouse_mapping_is_reversible() {
    let output = row("אב 界 גד", 20);
    assert_eq!(text(&output), "דג 界 בא");
    assert_eq!(output.glyphs.iter().map(|g| g.width).sum::<u16>(), 20);
    let wide = output.glyphs.iter().find(|g| g.text == "界").unwrap();
    assert_eq!(wide.width, 2);
    for logical in 0..8 {
        let visual = output.logical_to_visual[logical];
        assert_eq!(
            output.visual_to_logical[usize::from(visual)],
            logical as u16
        );
    }
}

#[test]
fn colours_travel_with_hebrew_letters() {
    let output = row("\x1b[31mאב\x1b[32mגד\x1b[0m", 20);
    assert_eq!(text(&output), "דגבא");
    assert_eq!(output.glyphs[0].style.fg, vt100::Color::Idx(2));
    assert_eq!(output.glyphs[3].style.fg, vt100::Color::Idx(1));
}

#[test]
fn cursor_maps_to_visual_letter_and_end_of_rtl_input() {
    let output = row("> אבגד", 20);
    assert_eq!(output.logical_to_visual[2], 5);
    assert_eq!(output.logical_to_visual[5], 2);
    assert_eq!(output.logical_to_visual[6], 2);
}

#[test]
fn fragmented_utf8_and_ansi_do_not_corrupt_the_logical_screen() {
    let source = "\x1b[31mשלום\x1b[0m עולם!".as_bytes();
    let mut whole = vt100::Parser::new(4, 40, 20);
    whole.process(source);
    let mut fragmented = vt100::Parser::new(4, 40, 20);
    let mut renderer = Renderer::default();
    let mut host = vt100::Parser::new(4, 40, 0);
    for byte in source {
        fragmented.process(&[*byte]);
        let mut output = Vec::new();
        renderer
            .render(fragmented.screen(), true, Direction::Auto, &mut output)
            .unwrap();
        host.process(&output);
    }
    assert_eq!(fragmented.screen().contents(), whole.screen().contents());
    assert_eq!(host.screen().contents(), "!םלוע םולש");
    assert_eq!(
        host.screen().cell(0, 8).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
}

#[test]
fn redraw_erase_and_alternate_screen_round_trip() {
    let mut logical = vt100::Parser::new(4, 30, 20);
    let mut host = vt100::Parser::new(4, 30, 0);
    let mut renderer = Renderer::default();
    for bytes in [
        "שלום עולם",
        "\r\x1b[2Kחדש",
        "\x1b[?1049hתפריט",
        "\x1b[?1049l",
    ] {
        logical.process(bytes.as_bytes());
        let mut output = Vec::new();
        renderer
            .render(logical.screen(), true, Direction::Auto, &mut output)
            .unwrap();
        host.process(&output);
    }
    assert_eq!(logical.screen().contents(), "חדש");
    assert_eq!(host.screen().contents(), "שדח");
    let mut unchanged = Vec::new();
    renderer
        .render(logical.screen(), true, Direction::Auto, &mut unchanged)
        .unwrap();
    assert!(unchanged.is_empty(), "an idle frame should emit no bytes");
}

#[test]
fn wrap_resize_and_toggle_keep_rows_in_bounds() {
    let mut parser = vt100::Parser::new(3, 8, 20);
    parser.process("אבגדהוזחטיכלמנסע".as_bytes());
    let mut renderer = Renderer::default();
    let mut output = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut output)
        .unwrap();
    let mut host = vt100::Parser::new(3, 8, 0);
    host.process(&output);
    assert_eq!(
        host.screen().rows(0, 8).take(2).collect::<Vec<_>>(),
        ["חזוהדגבא", "עסנמלכיט"]
    );
    output.clear();
    renderer
        .render(parser.screen(), false, Direction::Auto, &mut output)
        .unwrap();
    host.process(&output);
    assert_eq!(
        host.screen().rows(0, 8).collect::<Vec<_>>(),
        parser.screen().rows(0, 8).collect::<Vec<_>>()
    );
    parser.screen_mut().set_size(2, 5);
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut Vec::new())
        .unwrap();
}

#[test]
fn scrollback_view_does_not_change_live_content() {
    let mut parser = vt100::Parser::new(2, 20, 20);
    parser.process("אחד\r\nשתיים\r\nשלוש\r\nארבע".as_bytes());
    let live = parser.screen().contents();
    let mut renderer = Renderer::default();
    let mut bytes = Vec::new();
    renderer
        .render_scrollback(parser.screen(), 2, true, Direction::Auto, &mut bytes)
        .unwrap();
    let mut host = vt100::Parser::new(2, 20, 0);
    host.process(&bytes);
    assert!(host.screen().contents().contains("דחא"));
    assert_eq!(parser.screen().contents(), live);
}

#[test]
fn top_scroll_region_keeps_resumed_transcript_above_the_composer() {
    let mut parser = vt100::Parser::new(5, 30, 100);
    parser.process(b"RESTORED_0\r\nRESTORED_1\r\nRESTORED_2\r\nRESTORED_3\r\nDRAFT");
    parser.process(b"\x1b[1;4r\x1b[4;1H\nRESTORED_4\x1b[r");
    assert_eq!(parser.screen().rows(0, 30).nth(4).unwrap(), "DRAFT");
    parser.screen_mut().set_scrollback(100);
    assert_eq!(parser.screen().scrollback(), 1);
    assert_eq!(parser.screen().rows(0, 30).next().unwrap(), "RESTORED_0");
}

#[test]
fn inline_history_keeps_shifted_rows_cached() {
    let mut parser = vt100::Parser::new(4, 20, 100);
    parser.process(b"ROW_A\r\nROW_B\r\nROW_C\r\nROW_D");
    let mut renderer = Renderer::default();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut Vec::new())
        .unwrap();

    let mut history = Vec::new();
    parser
        .process_with_history(b"\r\nROW_E", |rows| {
            renderer.append_history(rows, (20, 4), true, Direction::Auto, &mut history)
        })
        .unwrap();
    let mut redraw = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut redraw)
        .unwrap();

    let redraw = String::from_utf8(redraw).unwrap();
    assert!(redraw.contains("ROW_E"));
    assert!(
        !redraw.contains("ROW_B"),
        "shifted rows must not be redrawn"
    );
}
