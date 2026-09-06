mod session;

use std::{
    ffi::OsString,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use terminal_rtl::display::Direction;

#[derive(Parser, Debug)]
#[command(
    name = "rtl",
    version,
    about = "Run a command with Hebrew display correction in your existing terminal.",
    after_help = "Examples:\n  rtl codex\n  rtl --demo\n  rtl powershell -NoLogo\n  rtl -- zsh\n\nDuring a session: Ctrl+] then r toggles correction; Ctrl+] then q exits.\nShift+PageUp / Shift+PageDown browse scrollback. Ctrl+] twice sends Ctrl+]."
)]
struct Args {
    /// Run the built-in Hebrew display and input demo.
    #[arg(long, conflicts_with = "command")]
    demo: bool,
    /// Check the local terminal without launching a command.
    #[arg(long, conflicts_with_all = ["command", "demo"])]
    doctor: bool,
    /// Start with visual correction disabled (toggle with Ctrl+] then r).
    #[arg(long)]
    no_bidi: bool,
    /// Base direction for each text field; auto detects Hebrew/English.
    #[arg(long, value_enum, default_value_t = Direction::Auto)]
    direction: Direction,
    /// Maximum retained scrollback rows (normal child screen only).
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u16).range(0..=50000))]
    scrollback: u16,
    /// Save the child's original PTY output, including ANSI, to a NEW file.
    #[arg(long, value_name = "PATH")]
    record: Option<PathBuf>,
    /// Do not replay retained output into normal scrollback when the command exits.
    #[arg(long)]
    no_replay: bool,
    /// Format recognizable Markdown tables/headings without changing the native UI.
    #[arg(long)]
    pretty: bool,
    /// Show you:/agent: labels beside recognized native message markers.
    #[arg(long, value_parser = ["codex", "grok"])]
    agent_label: Option<String>,
    #[arg(long, hide = true)]
    demo_child: bool,
    /// Command and arguments, passed directly without shell evaluation.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<OsString>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.min(255) as u8),
        Err(error) => {
            eprintln!("rtl: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<u32> {
    let mut args = Args::parse();
    if args.demo_child {
        return demo();
    }
    if args.doctor {
        println!("rtl {}", env!("CARGO_PKG_VERSION"));
        println!(
            "Platform: {} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        println!(
            "Terminal: {}",
            std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unknown".into())
        );
        println!(
            "Interactive input/output: {} / {}",
            io::stdin().is_terminal(),
            io::stdout().is_terminal()
        );
        println!(
            "PTY backend: {}",
            if cfg!(windows) {
                "Windows ConPTY"
            } else {
                "Unix PTY"
            }
        );
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            println!("Size: {cols} columns x {rows} rows");
        }
        println!("Visual test: rtl --demo");
        return Ok(0);
    }
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "an interactive terminal is required; run this in VS Code's terminal panel"
    );
    ensure!(
        std::env::var_os("RTL_ACTIVE").is_none(),
        "already inside an rtl session; launch the agent directly to avoid double correction"
    );
    if args.demo {
        args.command = vec![
            std::env::current_exe()?.into_os_string(),
            "--demo-child".into(),
        ];
    }
    ensure!(
        !args.command.is_empty(),
        "provide a command, for example: rtl codex (or try rtl --demo)"
    );
    let recording = args
        .record
        .as_ref()
        .map(|path| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
                .open(path)
                .with_context(|| format!("cannot create recording {}", path.display()))
        })
        .transpose()?;
    session::run(&args, recording)
}

fn demo() -> Result<u32> {
    println!("RTL demo — the agent's text stays in its original order.");
    println!("Ctrl+] then r: toggle correction. Ctrl+] then q: exit.");
    println!();
    println!("שלום עולם! איך אפשר לעזור לך היום?");
    println!("עברית עם English, המספר 123, וקובץ src/main.rs");
    println!("\x1b[32mטקסט ירוק\x1b[0m וגם שָׁלוֹם עם ניקוד");
    println!("│ שלום עולם  │  status: ready │");
    println!("Code: cargo test --locked");
    println!("| Name | Status |\r\n| ---- | ------ |\r\n| שלום | ready  |");
    print!("Streaming: ");
    io::stdout().flush()?;
    for c in "הטקסט מגיע בהדרגה, בלי להפוך את הקוד.".chars() {
        print!("{c}");
        io::stdout().flush()?;
        std::thread::sleep(std::time::Duration::from_millis(35));
    }
    println!("\n\nType Hebrew and press Enter. Type exit to finish.");
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 || line.trim() == "exit" {
            break;
        }
        println!("You entered: {}", line.trim_end());
    }
    Ok(0)
}
