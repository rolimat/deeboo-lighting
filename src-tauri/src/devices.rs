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

#[must_use]
pub fn audio_inputs() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}
