use super::harness::PtyHarness;

#[test]
fn interactive_npm_launcher_uses_wrapper_and_preserves_exit_status() {
    let mut harness = PtyHarness::new("interactive-launcher");
    harness.until("LAUNCHER_READY");
    harness.send(b"x");
    assert_eq!(harness.finish(), 9);
}

#[cfg(windows)]
#[test]
fn native_windows_npm_shim_interactive() {
    let Some(directory) = std::env::var_os("RTL_TEST_WINDOWS_NPM_BIN") else {
        return;
    };
    let shim = std::path::PathBuf::from(directory).join("codex.cmd");
    let args = ["שלום".into(), "space value".into(), "O'Brien".into()];
    let mut harness = PtyHarness::with_command(
        "windows-npm",
        &["--no-bidi".into()],
        shim.as_os_str(),
        &args,
    );
    harness.until("WINDOWS_ARGS_");
    harness.send(b"x");
    assert_eq!(harness.finish(), 7);
    let output = String::from_utf8_lossy(&harness.output);
    assert!(output.contains("שלום"), "{output}");
    assert!(output.contains("space value"), "{output}");
    assert!(output.contains("O'Brien"), "{output}");
}
