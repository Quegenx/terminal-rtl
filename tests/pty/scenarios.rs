use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

pub(super) fn run_fixture() {
    let Ok(scenario) = std::env::var("RTL_TEST_SCENARIO") else {
        return;
    };
    assert_eq!(
        std::env::current_dir().unwrap(),
        std::path::PathBuf::from(std::env::var_os("RTL_TEST_EXPECTED_CWD").unwrap())
    );
    let mut out = std::io::stdout();
    out.write_all(b"\x1b[2J\x1b[H").unwrap();
    match scenario.as_str() {
        "shutdown" | "shutdown-launcher" => {
            enable_fixture_raw_mode();
            write!(out, "CHILD_PID_{}_READY", std::process::id()).unwrap();
            out.flush().unwrap();
            let mut key = [0];
            super::fixture_input::read_fixture_bytes(&mut key).unwrap();
            std::process::exit(0);
        }
        "paste-boundary" => {
            enable_fixture_raw_mode();
            out.write_all(b"\x1b[?2004hPASTE_BOUNDARY_READY").unwrap();
            out.flush().unwrap();
            // Host closer ends the paste. Following bytes are ordinary key
            // events; the wire protocol cannot encode a literal closer.
            let expected = b"\x1b[200~safe\x1b[200~inside\x1b[201~AFTER";
            let mut bytes = vec![0; expected.len()];
            super::fixture_input::read_fixture_bytes(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
            std::process::exit(0);
        }
        "resize-replay" | "resize-no-replay" => {
            enable_fixture_raw_mode();
            for line in 0..20 {
                write!(out, "GEOMETRY_{line:03}\r\n").unwrap();
            }
            out.write_all(b"GEOMETRY_READY").unwrap();
            out.flush().unwrap();
            let mut key = [0];
            for dimensions in [(4, 2), (1, 1), (70, 13)] {
                super::fixture_input::trace_geometry(dimensions, "before input");
                super::fixture_input::read_fixture_bytes(&mut key).unwrap();
                super::fixture_input::trace_geometry(dimensions, "received input");
                let deadline = Instant::now() + Duration::from_secs(3);
                while crossterm::terminal::size().unwrap() != dimensions
                    && Instant::now() < deadline
                {
                    thread::sleep(Duration::from_millis(10));
                }
                assert_eq!(crossterm::terminal::size().unwrap(), dimensions);
                super::fixture_input::trace_geometry(dimensions, "before wide output");
                out.write_all("ab界\x1b[H\x1b[X界abc".as_bytes()).unwrap();
                out.flush().unwrap();
                super::fixture_input::trace_geometry(dimensions, "after wide output");
            }
            out.write_all(b"\r\nGEOMETRY_OK").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "interactive-launcher" => {
            assert_eq!(crossterm::terminal::size().unwrap(), (52, 11));
            enable_fixture_raw_mode();
            out.write_all(b"LAUNCHER_READY").unwrap();
            out.flush().unwrap();
            let mut key = [0];
            super::fixture_input::read_fixture_bytes(&mut key).unwrap();
            assert_eq!(key, [b'x']);
            std::process::exit(9);
        }
        "inline-burst" => {
            enable_fixture_raw_mode();
            for line in 0..100 {
                write!(out, "BURST_{line:03}\r\n").unwrap();
            }
            out.write_all(b"\x1b[3J\x1bcAFTER_RESET\r\nRESET_2\r\nRESET_3\r\nRESET_4\r\nRESET_5\r\nRESET_6\r\nRESET_7\r\nRESET_8\r\nRESET_9\r\nRESET_10\r\nBURST_READY").unwrap();
            out.flush().unwrap();
            let mut key = [0];
            super::fixture_input::read_fixture_bytes(&mut key).unwrap();
            std::process::exit(0);
        }
        "shift-enter" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            out.write_all(b"SHIFT_ENTER_READY").unwrap();
            out.flush().unwrap();
            #[cfg(unix)]
            {
                let mut shifted = [0; 7];
                super::fixture_input::read_fixture_bytes(&mut shifted).unwrap();
                assert_eq!(&shifted, b"\x1b[13;2u", "Shift+Enter must not submit");
            }
            #[cfg(windows)]
            assert_fixture_enter(true);
            out.write_all(b"\r\nNEWLINE_RECEIVED").unwrap();
            out.flush().unwrap();
            #[cfg(unix)]
            {
                let mut enter = [0];
                super::fixture_input::read_fixture_bytes(&mut enter).unwrap();
                assert_eq!(enter, [b'\r'], "plain Enter still submits");
            }
            #[cfg(windows)]
            assert_fixture_enter(false);
            std::process::exit(0);
        }
        "inline-history" => {
            enable_fixture_raw_mode();
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
            super::fixture_input::read_fixture_bytes(&mut key).unwrap();
            assert_eq!(key, [b'x']);
            std::process::exit(0);
        }
        "wheel" => {
            enable_fixture_raw_mode();
            for line in 0..50 {
                write!(out, "WHEEL_HISTORY_{line:03}\r\n").unwrap();
            }
            out.write_all(b"SCROLL_READY draft").unwrap();
            out.flush().unwrap();
            let mut typed = [0];
            super::fixture_input::read_fixture_bytes(&mut typed).unwrap();
            assert_eq!(typed, [b'x'], "scrolling must not send prompt-history keys");
            out.write_all(b"\r\n\x1b[?1000h\x1b[?1006hNATIVE_MOUSE_READY")
                .unwrap();
            out.flush().unwrap();
            let mut wheel = [0; 10];
            super::fixture_input::read_fixture_bytes(&mut wheel).unwrap();
            assert_eq!(&wheel, b"\x1b[<65;4;5M");
            out.write_all(b"\r\nWHEEL_OK").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "delete" => {
            enable_fixture_raw_mode();
            out.write_all(b"DELETE_READY").unwrap();
            out.flush().unwrap();
            let mut bytes = [0; 5];
            super::fixture_input::read_fixture_bytes(&mut bytes).unwrap();
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
            enable_fixture_raw_mode();
            out.write_all("שלום\x1b[6n".as_bytes()).unwrap();
            out.flush().unwrap();
            let mut query = [0; 6];
            super::fixture_input::read_fixture_bytes(&mut query).unwrap();
            assert_eq!(&query, b"\x1b[1;5R");
            out.write_all(b"\r\nQUERY_OK\r\n\x1b[?2004h").unwrap();
            out.flush().unwrap();
            let expected = "\x1b[200~שלום English 123\x1b[201~".as_bytes();
            let mut paste = vec![0; expected.len()];
            super::fixture_input::read_fixture_bytes(&mut paste).unwrap();
            assert_eq!(paste, expected);
            out.write_all(b"PASTE_OK\r\nRESIZE_READY\r\n").unwrap();
            out.flush().unwrap();
            let mut trigger = [0];
            super::fixture_input::read_fixture_bytes(&mut trigger).unwrap();
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
            super::fixture_input::read_fixture_bytes(&mut trigger).unwrap();
            assert_eq!(trigger, [3]);
            out.write_all(b"CTRL_C_OK\r\n").unwrap();
            out.flush().unwrap();
            crossterm::terminal::disable_raw_mode().unwrap();
            std::process::exit(7);
        }
        "toggle" => {
            enable_fixture_raw_mode();
            out.write_all("שלום\r\nTOGGLE_READY".as_bytes()).unwrap();
            out.flush().unwrap();
            let mut byte = [0];
            super::fixture_input::read_fixture_bytes(&mut byte).unwrap();
            assert_eq!(byte, [0x1d]);
            out.write_all(b"\r\nPREFIX_OK\r\n").unwrap();
            out.flush().unwrap();
            let mut unknown = [0; 2];
            super::fixture_input::read_fixture_bytes(&mut unknown).unwrap();
            assert_eq!(unknown, [0x1d, b'x']);
            std::process::exit(0);
        }
        _ => panic!("unknown fixture"),
    }
}

fn enable_fixture_raw_mode() {
    crossterm::terminal::enable_raw_mode().unwrap();
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            ENABLE_VIRTUAL_TERMINAL_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE,
            SetConsoleMode,
        };
        // These fixtures read VT bytes, unlike applications using ReadConsoleInput.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            assert_ne!(GetConsoleMode(handle, &mut mode), 0);
            assert_ne!(
                SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_INPUT),
                0
            );
        }
    }
}

#[cfg(windows)]
fn assert_fixture_enter(shift: bool) {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    loop {
        assert!(
            event::poll(Duration::from_secs(3)).unwrap(),
            "missing Enter"
        );
        if let Event::Key(key) = event::read().unwrap()
            && key.kind == KeyEventKind::Press
        {
            assert_eq!(key.code, KeyCode::Enter);
            assert_eq!(key.modifiers.contains(KeyModifiers::SHIFT), shift);
            break;
        }
    }
}
