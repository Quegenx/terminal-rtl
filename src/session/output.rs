use std::{
    io::{self, Read},
    sync::mpsc,
    thread,
};

pub(super) enum ChildOutput {
    Data(Vec<u8>),
    Error(io::Error),
}

pub(super) fn read_child_output(mut reader: Box<dyn Read + Send>) -> mpsc::Receiver<ChildOutput> {
    // Bound queued bytes (~512 KiB), so a noisy child cannot grow memory without limit.
    let (send, receive) = mpsc::sync_channel(64);
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if send
                        .send(ChildOutput::Data(buffer[..count].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                // Unix PTYs commonly report EIO when the last slave closes.
                Err(e) if cfg!(unix) && e.raw_os_error() == Some(5) => break,
                Err(e) => {
                    let _ = send.send(ChildOutput::Error(e));
                    break;
                }
            }
        }
    });

    receive
}
