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
