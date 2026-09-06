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
