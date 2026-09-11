use terminal_rtl::display::{Direction, Renderer};

fn render(parser: &vt100::Parser, renderer: &mut Renderer) -> vt100::Parser {
    let mut bytes = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut bytes)
        .unwrap();
    let (rows, cols) = parser.screen().size();
    let mut host = vt100::Parser::new(rows, cols, 0);
    host.process(&bytes);
    host
}

#[test]
fn table_decoration_preserves_native_status_prompt_columns_and_logical_text() {
    let mut parser = vt100::Parser::new(8, 40, 0);
    parser.process("| Name | Value |\r\n| ---- | ----- |\r\n| שלום | ready |\r\n\r\nSTATUS: model / context\r\n> ".as_bytes());
    let original = parser.screen().contents();
    let mut renderer = Renderer::default();
    renderer.set_pretty(true);
    let host = render(&parser, &mut renderer);
    assert!(host.screen().contents().contains("│ םולש │ ready │"));
    assert!(host.screen().contents().contains("├──────┼───────┤"));
    assert!(host.screen().cell(0, 2).unwrap().bold());
    assert_eq!(
        host.screen().rows(0, 40).nth(4).unwrap(),
        "STATUS: model / context"
    );
    assert_eq!(
        host.screen().cursor_position(),
        parser.screen().cursor_position()
    );
    assert_eq!(parser.screen().contents(), original);
    assert_eq!(renderer.logical_column(2, 7), 7);
}

#[test]
fn incomplete_tables_code_and_active_input_are_not_reinterpreted() {
    for input in [
        "x | y\r\n--|--\r\nz | w",
        "| A | B |\r\n| --- | --- |",
        "```\r\n| A   | B   |\r\n| --- | --- |\r\n| C   | D   |\r\n```",
    ] {
        let mut parser = vt100::Parser::new(10, 40, 0);
        parser.process(input.as_bytes());
        let mut renderer = Renderer::default();
        renderer.set_pretty(true);
        let host = render(&parser, &mut renderer);
        assert!(!host.screen().contents().contains('┼'));
    }
    let mut parser = vt100::Parser::new(5, 40, 0);
    parser.process(b"| A   | B   |\r\n| --- | --- |\r\n| C   | D   |");
    let mut renderer = Renderer::default();
    renderer.set_pretty(true);
    assert!(
        !render(&parser, &mut renderer)
            .screen()
            .contents()
            .contains('┼')
    );
}

#[test]
fn streamed_table_detection_invalidates_unchanged_header_and_can_be_disabled() {
    let mut parser = vt100::Parser::new(8, 40, 0);
    let mut renderer = Renderer::default();
    renderer.set_pretty(true);
    parser.process(b"| A   | B   |\r\n");
    render(&parser, &mut renderer);
    parser.process(b"| --- | --- |\r\n| C   | D   |\r\n> ");
    let host = render(&parser, &mut renderer);
    assert!(host.screen().cell(0, 2).unwrap().bold());
    renderer.set_pretty(false);
    assert_eq!(
        render(&parser, &mut renderer)
            .screen()
            .rows(0, 40)
            .next()
            .unwrap(),
        "| A   | B   |"
    );
}

#[test]
fn markdown_hierarchy_lists_quotes_and_code_keep_native_geometry_and_colors() {
    let mut parser = vt100::Parser::new(14, 50, 0);
    parser.process(b"# Title\r\n## Section\r\n### Detail\r\n- List item\r\n> Quoted text\r\n```rs\r\nlet value = 1;\r\n```\r\n\x1b[31m## Native red\x1b[0m\r\n\r\n> ");
    let original = parser.screen().contents();
    let mut renderer = Renderer::default();
    renderer.set_pretty(true);
    let host = render(&parser, &mut renderer);
    let cell = |row, col| host.screen().cell(row, col).unwrap();
    assert!(cell(0, 2).bold() && cell(0, 2).underline());
    assert!(cell(1, 3).bold() && !cell(1, 3).underline());
    assert_ne!(cell(1, 3).fgcolor(), cell(2, 4).fgcolor());
    assert!(cell(3, 0).bold());
    assert!(!cell(3, 2).bold());
    assert!(cell(4, 2).italic());
    assert!(!cell(6, 0).dim());
    assert!(cell(5, 0).dim());
    assert_eq!(cell(8, 3).fgcolor(), vt100::Color::Idx(1));
    assert_eq!(parser.screen().contents(), original);
    assert_eq!(
        host.screen()
            .contents()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>(),
        original.lines().map(str::trim_end).collect::<Vec<_>>()
    );
    assert_eq!(
        host.screen().cursor_position(),
        parser.screen().cursor_position()
    );
}

#[test]
fn speaker_margin_keeps_native_text_cursor_and_mouse_coordinates() {
    for agent in ["codex", "grok"] {
        let mut parser = vt100::Parser::new(8, 32, 0);
        parser
            .process("› hello\r\n• שלום\r\nSTATUS model\r\n```\r\n• code\r\n```\r\n› ".as_bytes());
        let original = parser.screen().contents();
        let mut renderer = Renderer::default();
        renderer.set_agent_label(Some(agent));
        let mut bytes = Vec::new();
        renderer
            .render(parser.screen(), true, Direction::Auto, &mut bytes)
            .unwrap();
        let mut host = vt100::Parser::new(8, 40, 0);
        host.process(&bytes);
        assert!(
            host.screen()
                .rows(0, 40)
                .next()
                .unwrap()
                .starts_with("you:    › hello")
        );
        assert!(
            host.screen()
                .rows(0, 40)
                .nth(1)
                .unwrap()
                .starts_with(&format!("{:<8}• םולש", format!("{agent}:")))
        );
        assert!(
            host.screen()
                .rows(0, 40)
                .nth(2)
                .unwrap()
                .starts_with("        STATUS")
        );
        assert!(
            host.screen()
                .rows(0, 40)
                .nth(4)
                .unwrap()
                .starts_with("        • code")
        );
        assert_eq!(host.screen().cursor_position(), (6, 10));
        assert_eq!(renderer.logical_column(0, 10), 2);
        assert_eq!(renderer.logical_column(1, 10), 5);
        assert_eq!(parser.screen().contents(), original);
        parser.process(b"\x1b[H\x1b[2KSTATUS changed");
        bytes.clear();
        renderer
            .render(parser.screen(), true, Direction::Auto, &mut bytes)
            .unwrap();
        host.process(&bytes);
        assert!(
            host.screen()
                .rows(0, 40)
                .next()
                .unwrap()
                .starts_with("        STATUS changed")
        );
    }
}

#[test]
fn native_grok_transcript_and_boxed_composer_get_full_labels() {
    let mut parser = vt100::Parser::new(9, 72, 0);
    parser.process("     ❯ hello                         7:28 PM\r\n     ◆ Thought for 0.0s\r\n     hello                           7:28 PM\r\n     Worked for 2.2s\r\n  │ ❯                                │\r\n> Markdown quote\r\n".as_bytes());
    let grid = parser.screen().viewport_rows();
    let labels = terminal_rtl::pretty::speaker_labels(&grid, Some("grok"));
    assert_eq!(
        labels[..6],
        [
            Some("you:".into()),
            None,
            Some("grok:".into()),
            None,
            Some("you:".into()),
            None
        ]
    );
}

#[test]
fn attribution_stays_below_native_content_across_redraw_and_resize() {
    let mut parser = vt100::Parser::new(5, 40, 0);
    let mut renderer = Renderer::default();
    renderer.set_attribution(true);
    let mut host = vt100::Parser::new(6, 40, 0);
    for text in [
        "\x1b[2J\x1b[H› hello\x1b[5;1HNative status",
        "\x1b[2J\x1b[H› שלום\x1b[5;1HUpdated status",
    ] {
        parser.process(text.as_bytes());
        let mut bytes = Vec::new();
        renderer
            .render(parser.screen(), true, Direction::Auto, &mut bytes)
            .unwrap();
        host.process(&bytes);
        assert_eq!(
            host.screen().rows(0, 40).nth(5).unwrap(),
            "Powered by: Gal Havkin"
        );
        assert_eq!(
            host.screen().cursor_position(),
            parser.screen().cursor_position()
        );
        assert!(host.screen().rows(0, 40).nth(4).unwrap().contains("status"));
    }
    parser.screen_mut().set_size(3, 30);
    host.screen_mut().set_size(4, 30);
    let mut bytes = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut bytes)
        .unwrap();
    host.process(&bytes);
    assert_eq!(
        host.screen().rows(0, 30).nth(3).unwrap(),
        "Powered by: Gal Havkin"
    );
}

#[test]
fn unchanged_attribution_is_not_repainted() {
    let mut parser = vt100::Parser::new(3, 40, 0);
    let mut renderer = Renderer::default();
    renderer.set_attribution(true);
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut Vec::new())
        .unwrap();
    parser.process(b"update");
    let mut bytes = Vec::new();
    renderer
        .render(parser.screen(), true, Direction::Auto, &mut bytes)
        .unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("Powered by"));
}

#[test]
fn grok_minimal_labels_plain_replies_without_labeling_status_or_input_continuations() {
    let mut parser = vt100::Parser::new(16, 72, 0);
    parser.process("❯ hello\r\ncontinued user input\r\n\r\n┃◆ Thought for 0.2s\r\n┃thinking text\r\nhello\r\nWorked for 1.5s\r\n\r\n❯ second message\r\n\r\nplain reply\r\nminimal · /help\r\n❯\r\nGrok 4.6 (high) · ctrl+o transcript".as_bytes());
    let grid = parser.screen().viewport_rows();
    let labels = terminal_rtl::pretty::speaker_labels(&grid, Some("grok"));
    assert_eq!(labels[0].as_deref(), Some("you:"));
    assert_eq!(labels[1], None);
    assert_eq!(labels[3].as_deref(), Some("grok:"));
    assert_eq!(labels[5].as_deref(), Some("grok:"));
    assert_eq!(labels[6], None);
    assert_eq!(labels[8].as_deref(), Some("you:"));
    assert_eq!(labels[10].as_deref(), Some("grok:"));
    assert_eq!(labels[11], None);
    assert_eq!(labels[12].as_deref(), Some("you:"));
    assert_eq!(labels[13], None);
}

#[test]
fn timestamp_labels_require_valid_clock_boundaries() {
    for (clock, valid) in [
        ("00:00", true),
        ("23:59", true),
        ("1:05 PM", true),
        ("12:59 AM", true),
        ("24:00", false),
        ("23:60", false),
        ("00:30 AM", false),
        ("23:59 PM", false),
        ("+1:00", false),
        ("12:3", false),
    ] {
        let mut parser = vt100::Parser::new(1, 60, 0);
        parser.process(format!("message  {clock}").as_bytes());
        let labels =
            terminal_rtl::pretty::speaker_labels(&parser.screen().viewport_rows(), Some("grok"));
        assert_eq!(labels[0].is_some(), valid, "{clock}");
    }
}
