//! Names of the MIDI sources and audio inputs the user can pick from.

use cpal::traits::{DeviceTrait, HostTrait};
use midir::MidiInput;

use crate::midi::VIRTUAL_PORT as OWN_PORT;

#[must_use]
pub fn midi_sources() -> Vec<String> {
    let Ok(input) = MidiInput::new("Deeboo Lighting list") else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .filter(|n| n != OWN_PORT)
        .collect()
}

/// Only names: listing must never query a config or open a stream (that is `Capture::open`'s job).
trait InputHost {
    fn input_names(&self) -> Vec<String>;
}

impl InputHost for cpal::Host {
    fn input_names(&self) -> Vec<String> {
        self.input_devices()
            .map(|it| it.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }
}

fn list_inputs(host: &impl InputHost) -> Vec<String> {
    host.input_names()
}

#[must_use]
pub fn audio_inputs() -> Vec<String> {
    list_inputs(&cpal::default_host())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Offers names and nothing else: no way to ask a device for a config or a stream.
    struct Fake(Vec<&'static str>);

    impl InputHost for Fake {
        fn input_names(&self) -> Vec<String> {
            self.0.iter().map(|s| (*s).to_string()).collect()
        }
    }

    #[test]
    fn listing_inputs_never_opens_a_stream() {
        let names = list_inputs(&Fake(vec!["Built-in Microphone", "BlackHole 2ch"]));
        assert_eq!(names, ["Built-in Microphone", "BlackHole 2ch"]);
    }
}
