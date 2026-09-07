//! Handlers only notify the session; terminal I/O and child cleanup run normally.
#[cfg(unix)]
mod platform {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    pub struct Shutdown {
        signal: Arc<AtomicUsize>,
        registrations: Vec<signal_hook::SigId>,
    }
    impl Shutdown {
        pub fn install() -> std::io::Result<Self> {
            let mut shutdown = Self {
                signal: Arc::new(AtomicUsize::new(0)),
                registrations: Vec::new(),
            };
            for signal in [
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGHUP,
                signal_hook::consts::SIGINT,
            ] {
                shutdown
                    .registrations
                    .push(signal_hook::flag::register_usize(
                        signal,
                        shutdown.signal.clone(),
                        signal as usize,
                    )?);
            }
            Ok(shutdown)
        }
        pub fn exit_code(&self) -> Option<u32> {
            let signal = self.signal.load(Ordering::Relaxed);
            (signal != 0).then_some(128 + signal as u32)
        }
    }
    impl Drop for Shutdown {
        fn drop(&mut self) {
            for id in self.registrations.drain(..) {
                signal_hook::low_level::unregister(id);
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        sync::atomic::{AtomicBool, AtomicU32, Ordering},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
        SetConsoleCtrlHandler,
    };
    static CODE: AtomicU32 = AtomicU32::new(0);
    static CLEANED: AtomicBool = AtomicBool::new(false);
    // Windows invokes this on a separate thread. Close/logoff/shutdown terminate
    // the process on return, so allow normal cleanup a bounded opportunity first.
    unsafe extern "system" fn handler(event: u32) -> i32 {
        if !matches!(
            event,
            CTRL_C_EVENT
                | CTRL_BREAK_EVENT
                | CTRL_CLOSE_EVENT
                | CTRL_LOGOFF_EVENT
                | CTRL_SHUTDOWN_EVENT
        ) {
            return 0;
        }
        CODE.store(130, Ordering::Relaxed);
        if matches!(
            event,
            CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT
        ) {
            let deadline = Instant::now() + Duration::from_secs(4);
            while !CLEANED.load(Ordering::Acquire) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        1
    }
    pub struct Shutdown;
    impl Shutdown {
        pub fn install() -> std::io::Result<Self> {
            CODE.store(0, Ordering::Relaxed);
            CLEANED.store(false, Ordering::Release);
            // SAFETY: the callback has the required ABI and only accesses statics.
            if unsafe { SetConsoleCtrlHandler(Some(handler), 1) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self)
        }
        pub fn exit_code(&self) -> Option<u32> {
            let code = CODE.load(Ordering::Relaxed);
            (code != 0).then_some(code)
        }
    }
    impl Drop for Shutdown {
        fn drop(&mut self) {
            CLEANED.store(true, Ordering::Release);
            // SAFETY: unregister the same static callback installed above.
            unsafe {
                SetConsoleCtrlHandler(Some(handler), 0);
            }
        }
    }
}
pub use platform::Shutdown;
