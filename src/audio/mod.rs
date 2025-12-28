//! Audio synthesis and playback

mod effects;
mod engine;
mod filter;
mod lfo;
mod noise;
mod oscillator;

pub use effects::{Bitcrusher, Chorus, Delay, Distortion, DistortionType, EffectsChain, Phaser, Reverb, RingMod};
pub use engine::{AudioEngine, PortamentoMode};
pub use filter::FilterType;
pub use lfo::{LFODestination, LFOWaveform, LFO};
pub use noise::{NoiseGenerator, NoiseType};
pub use oscillator::Waveform;
