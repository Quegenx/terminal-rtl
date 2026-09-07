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
