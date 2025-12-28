//! Audio engine - manages audio output and synthesis

use super::oscillator::{self, Waveform};
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Maximum number of simultaneous voices
const MAX_VOICES: usize = 20;

/// A single synthesizer voice
#[derive(Clone)]
struct Voice {
    /// MIDI note number
    note: u8,
    /// Velocity (0-127)
    velocity: u8,
    /// Current phase (0.0 - 1.0)
    phase: f32,
    /// Envelope state
    envelope: EnvelopeState,
    /// Envelope level (0.0 - 1.0)
    envelope_level: f32,
    /// Whether this voice is active
    active: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            note: 0,
            velocity: 0,
            phase: 0.0,
            envelope: EnvelopeState::Idle,
            envelope_level: 0.0,
            active: false,
        }
    }
}

/// ADSR envelope state
#[derive(Clone, Copy, PartialEq)]
enum EnvelopeState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Envelope parameters extracted for borrow-checker friendliness
#[derive(Clone, Copy)]
struct EnvelopeParams {
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    sample_rate: f32,
}

/// Shared state between audio thread and main thread
struct SharedState {
    /// Voice pool
    voices: [Voice; MAX_VOICES],
    /// Master volume (0.0 - 1.0)
    volume: f32,
    /// Sample rate
    sample_rate: f32,
    /// Oscillator waveform
    waveform: Waveform,
    /// Detune in cents
    detune: f32,
    /// ADSR parameters (in seconds)
    attack: f32,
    decay: f32,
    sustain: f32, // level, not time
    release: f32,
}

impl SharedState {
    fn new(sample_rate: f32) -> Self {
        Self {
            voices: std::array::from_fn(|_| Voice::default()),
            volume: 0.8,
            sample_rate,
            waveform: Waveform::Sawtooth, // Classic synth default
            detune: 0.0,
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.3,
        }
    }
}

/// Audio engine that manages synthesis and playback
pub struct AudioEngine {
    /// Audio stream (kept alive)
    _stream: Stream,
    /// Shared state with audio thread
    state: Arc<parking_lot::RwLock<SharedState>>,
    /// Voice count for UI
    voice_count: Arc<AtomicUsize>,
}

impl AudioEngine {
    /// Create and start a new audio engine
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No output device available")?;

        log::info!("Using audio device: {}", device.name()?);

        let config = device.default_output_config()?;
        log::info!(
            "Audio config: {} Hz, {} channels, {:?}",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );

        let sample_rate = config.sample_rate().0 as f32;
        let state = Arc::new(parking_lot::RwLock::new(SharedState::new(sample_rate)));
        let voice_count = Arc::new(AtomicUsize::new(0));

        let stream = Self::build_stream(&device, &config.into(), state.clone(), voice_count.clone())?;
        stream.play()?;

        Ok(Self {
            _stream: stream,
            state,
            voice_count,
        })
    }

    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        state: Arc<parking_lot::RwLock<SharedState>>,
        voice_count: Arc<AtomicUsize>,
    ) -> Result<Stream> {
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0 as f32;

        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                Self::audio_callback(data, channels, sample_rate, &state, &voice_count);
            },
            |err| log::error!("Audio stream error: {}", err),
            None,
        )?;

        Ok(stream)
    }

    fn audio_callback(
        data: &mut [f32],
        channels: usize,
        _sample_rate: f32,
        state: &Arc<parking_lot::RwLock<SharedState>>,
        voice_count_out: &Arc<AtomicUsize>,
    ) {
        let mut state = state.write();
        let volume = state.volume;
        let sample_rate = state.sample_rate;
        let waveform = state.waveform;
        let detune_cents = state.detune;

        // Extract envelope params to avoid borrow issues
        let env_params = EnvelopeParams {
            attack: state.attack,
            decay: state.decay,
            sustain: state.sustain,
            release: state.release,
            sample_rate,
        };

        // Process each sample
        for frame in data.chunks_mut(channels) {
            let mut sample = 0.0f32;
            let mut active_count = 0usize;

            // Sum all active voices
            for voice in state.voices.iter_mut() {
                if !voice.active && voice.envelope == EnvelopeState::Idle {
                    continue;
                }

                active_count += 1;

                // Calculate frequency from MIDI note with detune
                let detune_factor = 2.0f32.powf(detune_cents / 1200.0);
                let freq = 440.0 * 2.0f32.powf((voice.note as f32 - 69.0) / 12.0) * detune_factor;
                let phase_inc = freq / sample_rate;

                // Generate waveform using band-limited oscillator
                let osc_sample = oscillator::generate_bandlimited(waveform, voice.phase, phase_inc);

                // Update phase
                voice.phase += phase_inc;
                if voice.phase >= 1.0 {
                    voice.phase -= 1.0;
                }

                // Process envelope
                let env_level = Self::process_envelope(voice, &env_params);

                // Add to mix with velocity scaling
                let velocity_scale = voice.velocity as f32 / 127.0;
                sample += osc_sample * env_level * velocity_scale;
            }

            // Apply master volume and soft clip
            sample *= volume;
            sample = Self::soft_clip(sample);

            // Output to all channels
            for ch in frame.iter_mut() {
                *ch = sample;
            }

            voice_count_out.store(active_count, Ordering::Relaxed);
        }
    }

    fn process_envelope(voice: &mut Voice, params: &EnvelopeParams) -> f32 {
        match voice.envelope {
            EnvelopeState::Idle => {
                voice.envelope_level = 0.0;
            }
            EnvelopeState::Attack => {
                let attack_rate = 1.0 / (params.attack * params.sample_rate);
                voice.envelope_level += attack_rate;
                if voice.envelope_level >= 1.0 {
                    voice.envelope_level = 1.0;
                    voice.envelope = EnvelopeState::Decay;
                }
            }
            EnvelopeState::Decay => {
                let decay_rate = 1.0 / (params.decay * params.sample_rate);
                voice.envelope_level -= decay_rate;
                if voice.envelope_level <= params.sustain {
                    voice.envelope_level = params.sustain;
                    voice.envelope = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                voice.envelope_level = params.sustain;
            }
            EnvelopeState::Release => {
                let release_rate = voice.envelope_level / (params.release * params.sample_rate);
                voice.envelope_level -= release_rate;
                if voice.envelope_level <= 0.001 {
                    voice.envelope_level = 0.0;
                    voice.envelope = EnvelopeState::Idle;
                    voice.active = false;
                }
            }
        }

        voice.envelope_level
    }

    /// Soft clipping to prevent harsh digital distortion
    fn soft_clip(sample: f32) -> f32 {
        if sample > 1.0 {
            1.0 - (-sample + 1.0).exp() * 0.5
        } else if sample < -1.0 {
            -1.0 + (sample + 1.0).exp() * 0.5
        } else {
            sample
        }
    }

    /// Trigger a note on
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let mut state = self.state.write();

        // Find an available voice or steal the oldest
        let voice_idx = state
            .voices
            .iter()
            .position(|v| !v.active && v.envelope == EnvelopeState::Idle)
            .unwrap_or_else(|| {
                // Voice stealing: find voice in release or lowest envelope
                state
                    .voices
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        a.envelope_level
                            .partial_cmp(&b.envelope_level)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });

        let voice = &mut state.voices[voice_idx];
        voice.note = note;
        voice.velocity = velocity;
        voice.phase = 0.0;
        voice.envelope = EnvelopeState::Attack;
        voice.envelope_level = 0.0;
        voice.active = true;

        log::debug!("Note on: {} vel={} voice={}", note, velocity, voice_idx);
    }

    /// Trigger a note off
    pub fn note_off(&mut self, note: u8) {
        let mut state = self.state.write();

        for voice in state.voices.iter_mut() {
            if voice.active && voice.note == note && voice.envelope != EnvelopeState::Release {
                voice.envelope = EnvelopeState::Release;
                voice.active = false;
                log::debug!("Note off: {}", note);
            }
        }
    }

    /// Stop all notes immediately
    pub fn panic(&mut self) {
        let mut state = self.state.write();
        for voice in state.voices.iter_mut() {
            voice.envelope = EnvelopeState::Release;
            voice.active = false;
        }
        log::info!("Panic: stopping all notes");
    }

    /// Set master volume
    pub fn set_volume(&mut self, volume: f32) {
        let mut state = self.state.write();
        state.volume = volume.clamp(0.0, 1.0);
    }

    /// Get current voice count
    pub fn voice_count(&self) -> usize {
        self.voice_count.load(Ordering::Relaxed)
    }

    /// Set oscillator waveform
    pub fn set_waveform(&mut self, waveform: Waveform) {
        let mut state = self.state.write();
        state.waveform = waveform;
    }

    /// Get current waveform
    pub fn waveform(&self) -> Waveform {
        self.state.read().waveform
    }

    /// Cycle to next waveform
    pub fn next_waveform(&mut self) {
        let mut state = self.state.write();
        state.waveform = state.waveform.next();
    }

    /// Cycle to previous waveform
    pub fn prev_waveform(&mut self) {
        let mut state = self.state.write();
        state.waveform = state.waveform.prev();
    }

    /// Set detune in cents
    pub fn set_detune(&mut self, cents: f32) {
        let mut state = self.state.write();
        state.detune = cents.clamp(-100.0, 100.0);
    }

    /// Get current detune
    pub fn detune(&self) -> f32 {
        self.state.read().detune
    }

    /// Stop the audio engine
    pub fn stop(&mut self) {
        self.panic();
    }
}
