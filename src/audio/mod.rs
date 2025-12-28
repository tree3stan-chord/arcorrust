//! Audio synthesis and playback

mod effects;
mod engine;
mod filter;
mod oscillator;

pub use effects::{Delay, Distortion, DistortionType, EffectsChain, Reverb};
pub use engine::AudioEngine;
pub use filter::FilterType;
pub use oscillator::Waveform;
