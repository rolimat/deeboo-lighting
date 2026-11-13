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
/// `newest` takes the stamp of the latest clock tick queued, so the engine can tell a stall
/// (ticks waiting) from a silent clock (nothing arrived).
pub fn forward(
    tx: &SyncSender<Input>,
    dropped: &AtomicU64,
    newest: &AtomicU64,
    clock: &MonoClock,
    msg: &[u8],
) {
    let Some(&status) = msg.first() else { return };
    let event = MidiEvent {
        at_us: clock.now_us(),
        status,
    };
    if tx.try_send(Input::Midi(event)).is_err() {
        dropped.fetch_add(1, Ordering::Relaxed);
    } else if status == 0xF8 {
        newest.store(event.at_us, Ordering::Relaxed);
    }
}

/// The OS list of MIDI sources (a fake in tests). Our own virtual port is never a source.
pub trait EndpointProvider {
    fn sources(&self) -> Vec<String>;
}

/// Re-lists and tells the engine only when the list differs from `last`.
pub fn notify_if_changed(
    provider: &impl EndpointProvider,
    last: &mut Vec<String>,
    tx: &SyncSender<Input>,
) {
    let now = provider.sources();
    if now != *last && tx.try_send(Input::SourcesChanged(now.clone())).is_ok() {
        // A full queue keeps `last` stale so the next notification retries.
        *last = now;
    }
}

#[cfg(target_os = "macos")]
mod watcher {
    use super::{EndpointProvider, VIRTUAL_PORT, notify_if_changed};
    use crate::runtime::Input;
    use coremidi::{Client, Notification, Sources};
    use std::sync::mpsc::SyncSender;

    struct CoreMidi;

    impl EndpointProvider for CoreMidi {
        fn sources(&self) -> Vec<String> {
            Sources
                .into_iter()
                .filter_map(|s| s.display_name())
                .filter(|n| n != VIRTUAL_PORT)
                .collect()
        }
    }

    /// `CoreMIDI` refreshes its per-process endpoint cache only through notifications delivered on
    /// a run loop; midir's clients live on threads without one, so this thread runs one.
    pub fn start(tx: SyncSender<Input>) {
        let spawned = std::thread::Builder::new()
            .name("midi-watch".into())
            .spawn(move || {
                let mut last = Vec::new();
                let tx2 = tx.clone();
                let client = Client::new_with_notifications(
                    "Deeboo Lighting watch",
                    move |n: &Notification| {
                        if matches!(
                            n,
                            Notification::ObjectAdded(_)
                                | Notification::ObjectRemoved(_)
                                | Notification::SetupChanged
                        ) {
                            notify_if_changed(&CoreMidi, &mut last, &tx2);
                        }
                    },
                );
                let Ok(_client) = client else {
                    log::warn!("MIDI watcher not started");
                    return;
                };
                // The first list, for sources that exist at launch.
                notify_if_changed(&CoreMidi, &mut Vec::new(), &tx);
                core_foundation::runloop::CFRunLoop::run_current();
            });
        if let Err(e) = spawned {
            log::warn!("MIDI watcher thread not started: {e}");
        }
    }
}

/// Starts the thread that reports sources appearing and disappearing (macOS only).
pub fn watch_sources(tx: &SyncSender<Input>) {
    #[cfg(target_os = "macos")]
    watcher::start(tx.clone());
    // Other hosts have no notification run loop: one listing at start.
    #[cfg(not(target_os = "macos"))]
    {
        struct Listing;
        impl EndpointProvider for Listing {
            fn sources(&self) -> Vec<String> {
                crate::devices::midi_sources()
            }
        }
        notify_if_changed(&Listing, &mut Vec::new(), tx);
    }
}

pub struct MidiIn {
    _virtual: Option<MidiInputConnection<()>>,
    source: Option<(String, MidiInputConnection<()>)>,
    tx: SyncSender<Input>,
    clock: MonoClock,
    dropped: Arc<AtomicU64>,
    newest: Arc<AtomicU64>,
}

impl MidiIn {
    /// Always creates the virtual destination; also connects `source` if it is present.
    #[must_use]
    pub fn start(source: Option<&str>, tx: SyncSender<Input>, clock: MonoClock) -> Self {
        let dropped = Arc::new(AtomicU64::new(0));
        let newest = Arc::new(AtomicU64::new(0));
        let virtual_in = MidiInput::new(CLIENT).ok().and_then(|input| {
            let (tx, dropped, newest) = (tx.clone(), Arc::clone(&dropped), Arc::clone(&newest));
            input
                .create_virtual(
                    VIRTUAL_PORT,
                    move |_, msg, ()| forward(&tx, &dropped, &newest, &clock, msg),
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
            newest,
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
        let (tx, dropped, newest, clock) = (
            self.tx.clone(),
            Arc::clone(&self.dropped),
            Arc::clone(&self.newest),
            self.clock,
        );
        input
            .connect(
                &port,
                "Deeboo Lighting source",
                move |_, msg, ()| forward(&tx, &dropped, &newest, &clock, msg),
                (),
            )
            .map_err(|e| log::warn!("MIDI source not connected: {e}"))
            .ok()
    }
}

impl MidiPort for MidiIn {
    fn newest_tick_us(&self) -> u64 {
        self.newest.load(Ordering::Relaxed)
    }

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
        forward(&tx, &dropped, &AtomicU64::new(0), &clock, &[0xF8]);
        forward(&tx, &dropped, &AtomicU64::new(0), &clock, &[]);
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
            forward(&tx, &dropped, &AtomicU64::new(0), &clock, &[0xF8]);
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 3);
        assert_eq!(rx.try_iter().count(), 2);
    }

    struct Fake(std::cell::RefCell<Vec<String>>);
    impl EndpointProvider for Fake {
        fn sources(&self) -> Vec<String> {
            self.0.borrow().clone()
        }
    }

    #[test]
    fn watcher_emits_only_when_the_list_changes() {
        let (tx, rx) = sync_channel(8);
        let p = Fake(std::cell::RefCell::new(vec!["A".into()]));
        let mut last = Vec::new();
        notify_if_changed(&p, &mut last, &tx);
        notify_if_changed(&p, &mut last, &tx);
        p.0.borrow_mut().push("B".into());
        notify_if_changed(&p, &mut last, &tx);
        let got: Vec<_> = rx.try_iter().collect();
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[1], Input::SourcesChanged(l) if l == &["A", "B"]));
    }
}
