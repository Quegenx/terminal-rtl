//! Bounded synthetic input reads report the received bytes instead of hanging.
use std::{
    io::{self, Read},
    sync::mpsc,
    thread,
    time::Duration,
};

pub(super) fn read_fixture_bytes(bytes: &mut [u8]) -> io::Result<()> {
    let length = bytes.len();
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut input = io::stdin();
        for _ in 0..length {
            let mut byte = [0];
            let result = input.read_exact(&mut byte).map(|()| byte[0]);
            if send.send(result).is_err() {
                break;
            }
        }
    });
    for index in 0..length {
        bytes[index] = receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|_| {
                panic!(
                    "fixture expected {length} bytes, received {:?}",
                    &bytes[..index]
                )
            })?;
    }
    Ok(())
}

pub(super) fn trace_geometry(dimensions: (u16, u16), stage: &str) {
    use std::io::Write;
    if let Some(path) = std::env::var_os("RTL_TEST_GEOMETRY_TRACE") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{dimensions:?}: {stage}").unwrap();
    }
}

pub(super) fn report_native_trace(pid: Option<u32>) {
    if let Some(pid) = pid {
        let path = std::env::temp_dir().join(format!("rtl-native-input-{pid}.trace"));
        if std::thread::panicking()
            && let Ok(trace) = std::fs::read_to_string(&path)
        {
            eprintln!("NATIVE_RECORDS_START\n{trace}NATIVE_RECORDS_END");
        }
        let _ = std::fs::remove_file(path);
    }
}
