//! Debug probe: a virtual MIDI destination that counts clock bytes, to prove `CoreMIDI`
//! works inside the App Sandbox. Replaced by the real clock input in a later plan.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use midir::os::unix::VirtualInput;
use midir::{MidiInput, MidiInputConnection};

const CLOCK: u8 = 0xF8;
/// MIDI clock ticks per quarter note: the UI is refreshed once per beat.
const TICKS_PER_PUSH: u64 = 24;

/// Number of MIDI clock bytes in a message.
fn count_clock(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |n, &b| n + u64::from(b == CLOCK))
}

/// Creates the virtual input `name`; `on_beat` gets the running total every 24 ticks.
///
/// # Errors
/// Returns the `CoreMIDI` error text if the client or the port cannot be created.
pub fn start(
    name: &str,
    on_beat: impl Fn(u64) + Send + 'static,
) -> Result<MidiInputConnection<()>, String> {
    let input = MidiInput::new(name).map_err(|e| e.to_string())?;
    let total = Arc::new(AtomicU64::new(0));
    input
        .create_virtual(
            name,
            move |_, msg, ()| {
                let n = count_clock(msg);
                if n == 0 {
                    return;
                }
                let before = total.fetch_add(n, Ordering::Relaxed);
                let after = before + n;
                if before / TICKS_PER_PUSH != after / TICKS_PER_PUSH {
                    on_beat(after);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_clock_bytes() {
        assert_eq!(count_clock(&[0xF8, 0x90, 0x3C, 0x7F, 0xF8, 0xFA]), 2);
    }
}
