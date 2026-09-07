//! Real nested PTY journeys; modules share one bounded harness pool.
mod harness;
mod history;
mod input;
mod launchers;
mod lifecycle;
mod scenarios;

// Keep the child entry point stable for every harness scenario.
#[test]
fn fixture() {
    scenarios::run_fixture();
}

// Simulate a shell with real pre-existing output in the same native console.
#[test]
fn host_fixture() {
    use std::io::Write;
    let Ok(count) = std::env::var("RTL_TEST_HOST_ARG_COUNT") else {
        return;
    };
    let args: Vec<_> = (0..count.parse::<usize>().unwrap())
        .map(|index| std::env::var_os(format!("RTL_TEST_HOST_ARG_{index}")).unwrap())
        .collect();
    print!("EARLIER_SHELL_OUTPUT\r\n");
    std::io::stdout().flush().unwrap();
    let status = std::process::Command::new(&args[0])
        .args(&args[1..])
        .env_remove("RTL_TEST_HOST_ARG_COUNT")
        .status()
        .unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
