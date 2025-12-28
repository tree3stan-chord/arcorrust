//! Audio synthesis and playback

mod effects;
mod engine;
mod filter;
mod lfo;
mod noise;
mod oscillator;

// Public API re-exports - some may be unused within this crate but available for presets/external use
#[allow(unused_imports)]
pub use effects::{Bitcrusher, Chorus, Delay, Distortion, DistortionType, EffectsChain, Phaser, Reverb, RingMod};
pub use engine::{AudioEngine, PortamentoMode};
pub use filter::FilterType;
#[allow(unused_imports)]
pub use lfo::{LFODestination, LFOWaveform, LFO};
#[allow(unused_imports)]
pub use noise::{NoiseGenerator, NoiseType};
pub use oscillator::Waveform;
