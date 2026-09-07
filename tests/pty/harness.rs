//! Run the actual binary inside a real PTY, with this test executable as its child.
//! This exercises Unix PTYs locally and native ConPTY on the Windows CI runner.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Each harness nests two PTYs. Bound concurrency on hosts with small PTY pools.
static PTY_SLOTS: (std::sync::Mutex<usize>, std::sync::Condvar) =
    (std::sync::Mutex::new(0), std::sync::Condvar::new());
struct PtySlot;
impl PtySlot {
    fn acquire() -> Self {
        let (lock, ready) = &PTY_SLOTS;
        let mut count = lock.lock().unwrap();
        while *count >= 4 {
            count = ready.wait(count).unwrap();
        }
        *count += 1;
        Self
    }
}
impl Drop for PtySlot {
    fn drop(&mut self) {
        let (lock, ready) = &PTY_SLOTS;
        *lock.lock().unwrap() -= 1;
        ready.notify_one();
    }
}

pub(super) struct PtyHarness {
    _slot: PtySlot,
    pub(super) master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    pub(super) child: Box<dyn portable_pty::Child + Send + Sync>,
    receive: mpsc::Receiver<Vec<u8>>,
    pub(super) output: Vec<u8>,
    pub(super) host: vt100::Parser<terminal_rtl::protocol::Protocol>,
}

impl PtyHarness {
    pub(super) fn new(scenario: &str) -> Self {
        Self::with_options(scenario, &[])
    }

    pub(super) fn with_options(scenario: &str, options: &[std::ffi::OsString]) -> Self {
        Self::with_command(
            scenario,
            options,
            std::env::current_exe().unwrap().as_os_str(),
            &["--exact".into(), "fixture".into(), "--nocapture".into()],
        )
    }

    pub(super) fn with_command(
        scenario: &str,
        options: &[std::ffi::OsString],
        program: &std::ffi::OsStr,
        child_args: &[std::ffi::OsString],
    ) -> Self {
        let slot = PtySlot::acquire();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 12,
                cols: 60,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = if scenario.ends_with("-launcher") {
            let mut command = CommandBuilder::new(
                std::env::var("RTL_TEST_JS_RUNTIME").unwrap_or_else(|_| "node".into()),
            );
            let root = std::env::var_os("RTL_TEST_PACKAGE_ROOT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            command.arg(root.join("bin/terminal-rtl.mjs"));
            command.arg("codex");
            if std::env::var_os("RTL_TEST_PACKAGE_ROOT").is_none() {
                command.env("RTL_BIN", env!("CARGO_BIN_EXE_rtl"));
            } else {
                command.env_remove("RTL_BIN");
            }
            command.env("RTL_CODEX_BIN", program);
            command
        } else {
            CommandBuilder::new(env!("CARGO_BIN_EXE_rtl"))
        };
        let cwd = std::env::current_dir().unwrap();
        command.cwd(&cwd);
        command.env("RTL_TEST_EXPECTED_CWD", cwd);
        command.env("RTL_TEST_SCENARIO", scenario);
        command.env_remove("RTL_ACTIVE");
        command.args(options);
        if scenario == "inline-history" || scenario == "inline-burst" {
            command.args(["--inline", "--attribution", "--no-replay"]);
        }
        if scenario == "interactive-label" {
            command.args(["--pretty", "--agent-label", "codex", "--attribution"]);
        }
        if scenario == "inline-burst" {
            command.args(["--scrollback", "0"]);
        }
        if scenario == "resize-no-replay" {
            command.arg("--no-replay");
        }
        if scenario.starts_with("resize-") {
            command.args(["--agent-label", "codex", "--attribution"]);
        }
        if !scenario.ends_with("-launcher") {
            command.arg(program);
        }
        command.args(child_args);
        if matches!(scenario, "inline-history" | "inline-burst") {
            // Seed the actual console before rtl starts; an outer ConPTY can
            // clear a synthetic parser-only seed during its own initialization.
            let argv = command.get_argv().clone();
            command.env("RTL_TEST_HOST_ARG_COUNT", argv.len().to_string());
            for (index, argument) in argv.iter().enumerate() {
                command.env(format!("RTL_TEST_HOST_ARG_{index}"), argument);
            }
            *command.get_argv_mut() = vec![
                std::env::current_exe().unwrap().into(),
                "--exact".into(),
                "host_fixture".into(),
                "--nocapture".into(),
            ];
        }
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
            _slot: slot,
            master: pair.master,
            writer,
            child,
            receive,
            output: Vec::new(),
            host: vt100::Parser::new_with_callbacks(
                12,
                60,
                0,
                terminal_rtl::protocol::Protocol::default(),
            ),
        }
    }

    // A single incremental host parser answers ConPTY's initial cursor query
    // at (1,1), then tracks the real host cursor for subsequent queries.
    fn accept_output(&mut self, bytes: &[u8]) {
        self.host.process(bytes);
        let replies = std::mem::take(&mut self.host.callbacks_mut().replies);
        if !replies.is_empty() {
            self.send(&replies);
        }
        self.output.extend_from_slice(bytes);
    }

    pub(super) fn until(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            // ConPTY can repaint only a changed suffix instead of re-emitting a
            // contiguous string. Check its reconstructed screen as well as history.
            if String::from_utf8_lossy(&self.output).contains(needle)
                || self.host.screen().contents().contains(needle)
            {
                return;
            }
            if let Ok(bytes) = self.receive.recv_timeout(Duration::from_millis(20)) {
                self.accept_output(&bytes);
            }
        }
        panic!(
            "did not receive {needle:?}: {:?}",
            String::from_utf8_lossy(&self.output)
        );
    }

    pub(super) fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    pub(super) fn finish(&mut self) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            while let Ok(bytes) = self.receive.try_recv() {
                self.accept_output(&bytes);
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                // Give the reader a chance to drain bytes written just before exit.
                while let Ok(bytes) = self.receive.recv_timeout(Duration::from_millis(50)) {
                    self.accept_output(&bytes);
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

impl Drop for PtyHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn host_query_responses_are_incremental_and_exactly_once() {
    let mut host =
        vt100::Parser::new_with_callbacks(12, 60, 0, terminal_rtl::protocol::Protocol::default());
    let mut replies = Vec::new();
    for byte in b"\x1b[6n\x1b[4;8H\x1b[6ntext" {
        host.process(&[*byte]);
        replies.extend(std::mem::take(&mut host.callbacks_mut().replies));
    }
    assert_eq!(replies, b"\x1b[1;1R\x1b[4;8R");
}
