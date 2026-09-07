use super::harness::PtyHarness;
#[cfg(unix)]
use std::{
    thread,
    time::{Duration, Instant},
};

#[test]
fn real_pty_stream_exit_code_and_terminal_cleanup() {
    let mut harness = PtyHarness::new("stream");
    assert_eq!(harness.finish(), 17);
    let output = String::from_utf8_lossy(&harness.output);
    assert!(output.contains("!םלוע םולש"), "{output:?}");
    assert!(output.contains("STREAM_OK"));
    assert!(output.contains("\x1b[?1049l"));
    // ConPTY consumes wrap mode internally; it need not repeat that CSI outside.
    #[cfg(unix)]
    assert!(output.contains("\x1b[?7h"));
    assert!(output.contains("\x1b[?2004l"));
    assert!(!harness.host.screen().alternate_screen());
    assert!(!harness.host.screen().bracketed_paste());
    assert_eq!(
        harness.host.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::None
    );
}

#[cfg(unix)]
#[test]
fn sigterm_restores_terminal_and_terminates_child() {
    for scenario in ["shutdown", "shutdown-launcher"] {
        check_sigterm(scenario);
    }
}

#[cfg(unix)]
fn check_sigterm(scenario: &str) {
    let mut harness = PtyHarness::new(scenario);
    harness.until("_READY");
    let text = String::from_utf8_lossy(&harness.output);
    let pid = text
        .split("CHILD_PID_")
        .nth(1)
        .unwrap()
        .split('_')
        .next()
        .unwrap()
        .to_owned();
    let wrapper = harness.child.process_id().unwrap();
    assert!(
        std::process::Command::new("kill")
            .args(["-TERM", &wrapper.to_string()])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(harness.finish(), 143);
    let output = String::from_utf8_lossy(&harness.output);
    if scenario == "shutdown" {
        assert!(output.contains("\x1b[?1049l"));
    }
    assert!(output.contains("\x1b[?7h"));
    assert!(output.contains("\x1b[?2004l"));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive {
            break;
        }
        assert!(Instant::now() < deadline, "child {pid} survived SIGTERM");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn recordings_preserve_raw_text_and_refuse_overwrite() {
    let root = std::env::temp_dir().join(format!("rtl-record-test-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("capture");
    let result = std::panic::catch_unwind(|| {
        let options = [
            std::ffi::OsString::from("--record"),
            path.as_os_str().to_owned(),
        ];
        let mut harness = PtyHarness::with_options("stream", &options);
        assert_eq!(harness.finish(), 17);
        let original = std::fs::read(&path).unwrap();
        let mut recorded = vt100::Parser::new(12, 60, 100);
        recorded.process(&original);
        assert!(recorded.screen().contents().contains("שלום עולם!"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        drop(harness);
        let mut retry = PtyHarness::with_options("stream", &options);
        assert_eq!(retry.finish(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!String::from_utf8_lossy(&retry.output).contains("\x1b[?1049h"));
    });
    std::fs::remove_dir_all(root).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn command_startup_failure_does_not_leave_terminal_modes_enabled() {
    let mut harness = PtyHarness::with_options(
        "stream",
        &["--".into(), "rtl-audit-missing-executable-12345".into()],
    );
    assert_eq!(harness.finish(), 1);
    let output = String::from_utf8_lossy(&harness.output);
    assert!(output.contains("cannot launch"));
    assert!(!output.contains("\x1b[?1049h"));
}
