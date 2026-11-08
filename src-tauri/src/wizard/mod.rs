//! Connection wizard: pure state machine (`state`) and the Smart Life cloud steps (`cloud`).

pub mod cloud;
pub mod session;
pub mod state;

/// One capturing logger for every test that checks the logs for secrets. `log` accepts a single
/// logger per process, so those tests share it and take `LOCK` to keep their lines apart.
#[cfg(test)]
pub(crate) mod testlog {
    use std::sync::{Mutex, MutexGuard, PoisonError};

    struct Capture;
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, r: &log::Record) {
            LINES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(format!("{}", r.args()));
        }
        fn flush(&self) {}
    }

    static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static LOCK: Mutex<()> = Mutex::new(());

    /// Installs the logger and returns the guard that keeps other logger tests out; the capture starts empty.
    pub fn begin() -> MutexGuard<'static, ()> {
        let guard = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        log::set_logger(&Capture).ok();
        log::set_max_level(log::LevelFilter::Trace);
        LINES.lock().unwrap_or_else(PoisonError::into_inner).clear();
        guard
    }

    pub fn lines() -> String {
        LINES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .join("\n")
    }
}
