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

/// Shared clock counter: calls `on_beat` with the running total every 24 ticks.
fn counter(
    total: Arc<AtomicU64>,
    on_beat: Arc<dyn Fn(u64) + Send + Sync>,
) -> impl FnMut(u64, &[u8], &mut ()) + Send + 'static {
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
    }
}

/// Creates the virtual input `name` and listens to every MIDI source present now
/// (e.g. Deeboo MIDI Sync's output); all of them feed one clock count.
///
/// # Errors
/// Returns the `CoreMIDI` error text if the client or the virtual port cannot be created.
pub fn start(
    name: &str,
    on_beat: impl Fn(u64) + Send + Sync + 'static,
) -> Result<Vec<MidiInputConnection<()>>, String> {
    let total = Arc::new(AtomicU64::new(0));
    let on_beat: Arc<dyn Fn(u64) + Send + Sync> = Arc::new(on_beat);
    let virtual_in = MidiInput::new(name)
        .map_err(|e| e.to_string())?
        .create_virtual(name, counter(Arc::clone(&total), Arc::clone(&on_beat)), ())
        .map_err(|e| e.to_string())?;
    let mut conns = vec![virtual_in];
    let ports = MidiInput::new(name).map_err(|e| e.to_string())?.ports();
    for port in ports {
        let Ok(input) = MidiInput::new(name) else {
            continue;
        };
        let source = input.port_name(&port).unwrap_or_default();
        match input.connect(
            &port,
            name,
            counter(Arc::clone(&total), Arc::clone(&on_beat)),
            (),
        ) {
            Ok(conn) => {
                log::info!("MIDI probe listening to '{source}'");
                conns.push(conn);
            }
            Err(e) => log::warn!("MIDI probe cannot listen to '{source}': {e}"),
        }
    }
    Ok(conns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_clock_bytes() {
        assert_eq!(count_clock(&[0xF8, 0x90, 0x3C, 0x7F, 0xF8, 0xFA]), 2);
    }
}
