//! MIDI input: the virtual destination `Deeboo Lighting In` plus the chosen source.
//! The callback only timestamps and `try_send`s a `Copy` event: no lock, no allocation, no log.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;

use midir::os::unix::VirtualInput;
use midir::{MidiInput, MidiInputConnection};

use crate::clock::MonoClock;
use crate::runtime::{Input, MidiPort};

pub const VIRTUAL_PORT: &str = "Deeboo Lighting In";
const CLIENT: &str = "Deeboo Lighting";
/// Bounded: a stalled engine drops MIDI instead of growing memory.
pub const QUEUE: usize = 1024;

/// One MIDI message as the engine needs it: when it arrived and its status byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub at_us: u64,
    pub status: u8,
}

/// The callback body. A full (or closed) queue only bumps `dropped`.
pub fn forward(tx: &SyncSender<Input>, dropped: &AtomicU64, clock: &MonoClock, msg: &[u8]) {
    let Some(&status) = msg.first() else { return };
    let event = MidiEvent {
        at_us: clock.now_us(),
        status,
    };
    if tx.try_send(Input::Midi(event)).is_err() {
        dropped.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct MidiIn {
    _virtual: Option<MidiInputConnection<()>>,
    source: Option<(String, MidiInputConnection<()>)>,
    tx: SyncSender<Input>,
    clock: MonoClock,
    dropped: Arc<AtomicU64>,
}

impl MidiIn {
    /// Always creates the virtual destination; also connects `source` if it is present.
    #[must_use]
    pub fn start(source: Option<&str>, tx: SyncSender<Input>, clock: MonoClock) -> Self {
        let dropped = Arc::new(AtomicU64::new(0));
        let virtual_in = MidiInput::new(CLIENT).ok().and_then(|input| {
            let (tx, dropped) = (tx.clone(), Arc::clone(&dropped));
            input
                .create_virtual(
                    VIRTUAL_PORT,
                    move |_, msg, ()| forward(&tx, &dropped, &clock, msg),
                    (),
                )
                .map_err(|e| log::warn!("virtual MIDI port not created: {e}"))
                .ok()
        });
        let mut me = Self {
            _virtual: virtual_in,
            source: None,
            tx,
            clock,
            dropped,
        };
        me.reconnect(source);
        me
    }

    fn connect(&self, name: &str) -> Option<MidiInputConnection<()>> {
        let input = MidiInput::new(CLIENT).ok()?;
        let port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).is_ok_and(|n| n == name))?;
        let (tx, dropped, clock) = (self.tx.clone(), Arc::clone(&self.dropped), self.clock);
        input
            .connect(
                &port,
                "Deeboo Lighting source",
                move |_, msg, ()| forward(&tx, &dropped, &clock, msg),
                (),
            )
            .map_err(|e| log::warn!("MIDI source not connected: {e}"))
            .ok()
    }
}

impl MidiPort for MidiIn {
    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    fn connected_source(&self) -> Option<String> {
        self.source.as_ref().map(|(n, _)| n.clone())
    }

    fn source_available(&self, name: &str) -> bool {
        MidiInput::new(CLIENT).is_ok_and(|input| {
            input
                .ports()
                .iter()
                .any(|p| input.port_name(p).is_ok_and(|n| n == name))
        })
    }

    fn reconnect(&mut self, source: Option<&str>) {
        self.source = None;
        if let Some(name) = source {
            self.source = self.connect(name).map(|c| (name.to_string(), c));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn midi_callback_event_is_copy_and_timestamped() {
        assert_copy::<MidiEvent>();
        let (tx, rx) = sync_channel(4);
        let dropped = AtomicU64::new(0);
        let clock = MonoClock::new();
        forward(&tx, &dropped, &clock, &[0xF8]);
        forward(&tx, &dropped, &clock, &[]);
        let got = rx.try_recv().ok();
        assert!(
            matches!(got, Some(Input::Midi(e)) if e.status == 0xF8 && e.at_us <= clock.now_us())
        );
        assert!(rx.try_recv().is_err(), "an empty message sends nothing");
    }

    #[test]
    fn full_channel_drops_and_counts() {
        let (tx, rx) = sync_channel(2);
        let dropped = AtomicU64::new(0);
        let clock = MonoClock::new();
        for _ in 0..5 {
            forward(&tx, &dropped, &clock, &[0xF8]);
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 3);
        assert_eq!(rx.try_iter().count(), 2);
    }
}
