//! Run the actual binary inside a real PTY, with this test executable as its child.
//! This exercises Unix PTYs locally and native ConPTY on the Windows CI runner.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

struct Harness {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    receive: mpsc::Receiver<Vec<u8>>,
    output: Vec<u8>,
}

impl Harness {
    fn new(scenario: &str) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 12,
                cols: 60,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_rtl"));
        let cwd = std::env::current_dir().unwrap();
        command.cwd(&cwd);
        command.env("RTL_TEST_EXPECTED_CWD", cwd);
        command.env("RTL_TEST_SCENARIO", scenario);
        command.env_remove("RTL_ACTIVE");
        if scenario == "inline-history" {
            command.args(["--inline", "--attribution", "--no-replay"]);
        }
        if scenario == "interactive-label" {
            command.args(["--pretty", "--agent-label", "codex", "--attribution"]);
        }
        command.arg(std::env::current_exe().unwrap());
        command.args(["--exact", "fixture", "--nocapture"]);
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let mut buffer = [0; 4096];
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 || send.send(buffer[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            master: pair.master,
            writer,
            child,
            receive,
            output: Vec::new(),
        }
    }

    fn until(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if String::from_utf8_lossy(&self.output).contains(needle) {
                return;
            }
            if let Ok(bytes) = self.receive.recv_timeout(Duration::from_millis(20)) {
                self.output.extend(bytes);
            }
        }
        panic!(
            "did not receive {needle:?}: {:?}",
            String::from_utf8_lossy(&self.output)
        );
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    fn finish(&mut self) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            while let Ok(bytes) = self.receive.try_recv() {
                self.output.extend(bytes);
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                // Give the reader a chance to drain bytes written just before exit.
                while let Ok(bytes) = self.receive.recv_timeout(Duration::from_millis(50)) {
                    self.output.extend(bytes);
                }
                return status.exit_code();
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "wrapper failed to exit: {:?}",
            String::from_utf8_lossy(&self.output)
        );
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn fixture() {
    let Ok(scenario) = std::env::var("RTL_TEST_SCENARIO") else {
        return;
    };
    assert_eq!(
        std::env::current_dir().unwrap(),
        std::path::PathBuf::from(std::env::var_os("RTL_TEST_EXPECTED_CWD").unwrap())
    );
    let mut out = std::io::stdout();
    let mut input = std::io::stdin();
    out.write_all(b"\x1b[2J\x1b[H").unwrap();
    match scenario.as_str() {
        "shift-enter" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            out.write_all(b"SHIFT_ENTER_READY").unwrap();
            out.flush().unwrap();
            let mut shifted = [0; 7];
            input.read_exact(&mut shifted).unwrap();
            assert_eq!(&shifted, b"\x1b[13;2u", "Shift+Enter must not submit");
            out.write_all(b"\r\nNEWLINE_RECEIVED").unwrap();
            out.flush().unwrap();
            let mut enter = [0];
            input.read_exact(&mut enter).unwrap();
            assert_eq!(enter, [b'\r'], "plain Enter still submits");
            std::process::exit(0);
        }
        "inline-history" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            // Codex resume scrolls a top region while keeping its composer fixed.
            out.write_all(b"\x1b[1;8r\x1b[1;1H").unwrap();
            out.write_all(b"\x1b]8;id=resumed;https://example.com/complete/destination\x1b\\")
                .unwrap();
            for line in 0..50 {
                write!(out, "RESUMED_{line:03}\r\n").unwrap();
            }
            out.write_all(b"\x1b]8;;\x1b\\\x1b[r\x1b[10;1HINLINE_DRAFT_READY")
                .unwrap();
            out.flush().unwrap();
            let mut key = [0];
            input.read_exact(&mut key).unwrap();
            assert_eq!(key, [b'x']);
            std::process::exit(0);
        }
        "wheel" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            for line in 0..50 {
                write!(out, "WHEEL_HISTORY_{line:03}\r\n").unwrap();
            }
            out.write_all(b"SCROLL_READY draft").unwrap();
            out.flush().unwrap();
            let mut typed = [0];
            input.read_exact(&mut typed).unwrap();
            assert_eq!(typed, [b'x'], "scrolling must not send prompt-history keys");
            out.write_all(b"\r\n\x1b[?1000h\x1b[?1006hNATIVE_MOUSE_READY")
                .unwrap();
            out.flush().unwrap();
            let mut wheel = [0; 10];
            input.read_exact(&mut wheel).unwrap();
            assert_eq!(&wheel, b"\x1b[<65;4;5M");
            out.write_all(b"\r\nWHEEL_OK").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "delete" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            out.write_all(b"DELETE_READY").unwrap();
            out.flush().unwrap();
            let mut bytes = [0; 5];
            input.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"\x7f\x1b[3~");
            out.write_all(b"\r\nDELETE_OK").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "history" => {
            for line in 0..50 {
                writeln!(out, "HISTORY_{line:03}").unwrap();
            }
            out.flush().unwrap();
            std::process::exit(0);
        }
        "stream" => {
            for byte in "\x1b[32mשלום עולם!\x1b[0m\r\nSTREAM_OK\r\n".as_bytes() {
                out.write_all(&[*byte]).unwrap();
                out.flush().unwrap();
                thread::sleep(Duration::from_millis(1));
            }
            std::process::exit(17);
        }
        "interactive" | "interactive-label" => {
            let expected_rows = if scenario == "interactive-label" {
                13
            } else {
                14
            };
            let expected_cols = if scenario == "interactive-label" {
                62
            } else {
                70
            };
            assert_eq!(
                crossterm::terminal::size().unwrap(),
                (expected_cols - 10, expected_rows - 2)
            );
            crossterm::terminal::enable_raw_mode().unwrap();
            out.write_all("שלום\x1b[6n".as_bytes()).unwrap();
            out.flush().unwrap();
            let mut query = [0; 6];
            input.read_exact(&mut query).unwrap();
            assert_eq!(&query, b"\x1b[1;5R");
            out.write_all(b"\r\nQUERY_OK\r\n\x1b[?2004h").unwrap();
            out.flush().unwrap();
            let expected = "\x1b[200~שלום English 123\x1b[201~".as_bytes();
            let mut paste = vec![0; expected.len()];
            input.read_exact(&mut paste).unwrap();
            assert_eq!(paste, expected);
            out.write_all(b"PASTE_OK\r\nRESIZE_READY\r\n").unwrap();
            out.flush().unwrap();
            let mut trigger = [0];
            input.read_exact(&mut trigger).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            while crossterm::terminal::size().unwrap() != (expected_cols, expected_rows)
                && Instant::now() < deadline
            {
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                crossterm::terminal::size().unwrap(),
                (expected_cols, expected_rows)
            );
            out.write_all(b"RESIZE_OK\r\n").unwrap();
            out.flush().unwrap();
            input.read_exact(&mut trigger).unwrap();
            assert_eq!(trigger, [3]);
            out.write_all(b"CTRL_C_OK\r\n").unwrap();
            out.flush().unwrap();
            crossterm::terminal::disable_raw_mode().unwrap();
            std::process::exit(7);
        }
        "toggle" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            out.write_all("שלום\r\nTOGGLE_READY".as_bytes()).unwrap();
            out.flush().unwrap();
            let mut byte = [0];
            input.read_exact(&mut byte).unwrap();
            assert_eq!(byte, [0x1d]);
            out.write_all(b"\r\nPREFIX_OK\r\n").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn real_pty_shift_enter_preserves_modifier_and_restores_host_keyboard() {
    let mut harness = Harness::new("shift-enter");
    harness.until("SHIFT_ENTER_READY");
    assert!(String::from_utf8_lossy(&harness.output).contains("\x1b[>1u"));
    harness.send(b"\x1b[13;2u");
    harness.until("NEWLINE_RECEIVED");
    harness.send(b"\r");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("\x1b[<1u"));
}

#[test]
fn real_pty_backspace_and_forward_delete_remain_distinct() {
    let mut harness = Harness::new("delete");
    harness.until("DELETE_READY");
    harness.send(b"\x7f\x1b[3~");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("DELETE_OK"));
}

#[test]
fn real_pty_stream_exit_code_and_terminal_cleanup() {
    let mut harness = Harness::new("stream");
    assert_eq!(harness.finish(), 17);
    let output = String::from_utf8_lossy(&harness.output);
    assert!(output.contains("!םלוע םולש"), "{output:?}");
    assert!(output.contains("STREAM_OK"));
    assert!(output.contains("\x1b[?1049l"));
    assert!(output.contains("\x1b[?7h"));
    assert!(output.contains("\x1b[?2004l"));
}

#[test]
fn real_pty_queries_paste_resize_and_ctrl_c() {
    for scenario in ["interactive", "interactive-label"] {
        let mut harness = Harness::new(scenario);
        harness.until("QUERY_OK");
        harness.send("\x1b[200~שלום English 123\x1b[201~".as_bytes());
        harness.until("RESIZE_READY");
        harness
            .master
            .resize(PtySize {
                rows: 14,
                cols: 70,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        harness.send(b"s");
        harness.until("RESIZE_OK");
        harness.send(&[3]);
        assert_eq!(harness.finish(), 7);
        assert!(String::from_utf8_lossy(&harness.output).contains("CTRL_C_OK"));
    }
}

#[test]
fn real_pty_toggle_and_literal_prefix() {
    let mut harness = Harness::new("toggle");
    harness.until("TOGGLE_READY");
    assert!(String::from_utf8_lossy(&harness.output).contains("םולש"));
    harness.output.clear();
    harness.send(b"\x1dr");
    harness.until("שלום");
    harness.send(b"\x1d\x1d");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("PREFIX_OK"));
}

#[test]
fn retained_history_is_replayed_after_exit() {
    let mut harness = Harness::new("history");
    assert_eq!(harness.finish(), 0);
    let output = String::from_utf8_lossy(&harness.output);
    let replay = output.rsplit("\x1b[?1049l").next().unwrap();
    assert!(replay.contains("HISTORY_000"), "{replay:?}");
    assert!(replay.contains("HISTORY_049"));
}

#[test]
fn real_pty_wheel_browses_history_without_changing_draft_and_preserves_native_mouse() {
    let mut harness = Harness::new("wheel");
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
    let mut harness = Harness::new("inline-history");
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
