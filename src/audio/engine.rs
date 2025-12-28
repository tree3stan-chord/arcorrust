//! Audio engine - manages audio output and synthesis

use super::effects::EffectsChain;
use super::filter::{calculate_filter_coeffs, BiquadFilter, BiquadState, FilterType};
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
    /// Oscillator 1 phase (0.0 - 1.0)
    phase1: f32,
    /// Oscillator 2 phase (0.0 - 1.0)
    phase2: f32,
    /// Amplitude envelope state
    envelope: EnvelopeState,
    /// Amplitude envelope level (0.0 - 1.0)
    envelope_level: f32,
    /// Filter envelope state
    filter_envelope: EnvelopeState,
    /// Filter envelope level (0.0 - 1.0)
    filter_envelope_level: f32,
    /// Whether this voice is active
    active: bool,
    /// Filter state (per-voice for proper polyphony)
    filter_state: BiquadState,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            note: 0,
            velocity: 0,
            phase1: 0.0,
            phase2: 0.0,
            envelope: EnvelopeState::Idle,
            envelope_level: 0.0,
            filter_envelope: EnvelopeState::Idle,
            filter_envelope_level: 0.0,
            active: false,
            filter_state: BiquadState::default(),
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
    /// Oscillator 1 waveform
    osc1_waveform: Waveform,
    /// Oscillator 1 detune in cents
    osc1_detune: f32,
    /// Oscillator 2 enabled
    osc2_enabled: bool,
    /// Oscillator 2 waveform
    osc2_waveform: Waveform,
    /// Oscillator 2 detune in cents
    osc2_detune: f32,
    /// Oscillator 2 pitch offset in semitones
    osc2_pitch: i32,
    /// Oscillator mix (0.0 = all osc1, 1.0 = all osc2)
    osc_mix: f32,
    /// Filter
    filter: BiquadFilter,
    /// Amplitude ADSR parameters (in seconds)
    attack: f32,
    decay: f32,
    sustain: f32, // level, not time
    release: f32,
    /// Filter ADSR parameters (in seconds)
    filter_attack: f32,
    filter_decay: f32,
    filter_sustain: f32, // level, not time
    filter_release: f32,
    /// Filter envelope amount (0.0 to 1.0, how much envelope affects cutoff)
    filter_env_amount: f32,
    /// Effects chain (distortion, delay, reverb)
    effects: EffectsChain,
    /// Recording buffer (accumulates samples when recording)
    recording_buffer: Vec<f32>,
    /// Recording enabled flag
    recording_enabled: bool,
    /// Maximum recording samples (10 minutes)
    recording_max_samples: usize,
}

impl SharedState {
    fn new(sample_rate: f32) -> Self {
        Self {
            voices: std::array::from_fn(|_| Voice::default()),
            volume: 0.8,
            sample_rate,
            osc1_waveform: Waveform::Sawtooth,
            osc1_detune: 0.0,
            osc2_enabled: false,
            osc2_waveform: Waveform::Square,
            osc2_detune: 7.0, // Classic detune
            osc2_pitch: 0,
            osc_mix: 0.5, // 50/50 when osc2 enabled
            filter: BiquadFilter::new(sample_rate),
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.3,
            // Filter envelope - snappier by default for that classic sweep
            filter_attack: 0.001,
            filter_decay: 0.2,
            filter_sustain: 0.0,
            filter_release: 0.2,
            filter_env_amount: 0.5, // 50% envelope depth
            effects: EffectsChain::new(sample_rate),
            // Recording - max 10 minutes stereo at sample rate
            recording_buffer: Vec::new(),
            recording_enabled: false,
            recording_max_samples: (sample_rate as usize) * 60 * 10 * 2,
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

        // Extract oscillator params
        let osc1_waveform = state.osc1_waveform;
        let osc1_detune = state.osc1_detune;
        let osc2_enabled = state.osc2_enabled;
        let osc2_waveform = state.osc2_waveform;
        let osc2_detune = state.osc2_detune;
        let osc2_pitch = state.osc2_pitch;
        let osc_mix = state.osc_mix;

        // Extract filter coefficients to avoid borrow issues
        let filter_enabled = state.filter.enabled();
        let filter_base_cutoff = state.filter.cutoff();
        let filter_resonance = state.filter.resonance();
        let filter_type = state.filter.filter_type();
        let filter_env_amount = state.filter_env_amount;

        // Extract envelope params to avoid borrow issues
        let env_params = EnvelopeParams {
            attack: state.attack,
            decay: state.decay,
            sustain: state.sustain,
            release: state.release,
            sample_rate,
        };

        // Filter envelope params
        let filter_env_params = EnvelopeParams {
            attack: state.filter_attack,
            decay: state.filter_decay,
            sustain: state.filter_sustain,
            release: state.filter_release,
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

                // Base frequency from MIDI note
                let base_freq = 440.0 * 2.0f32.powf((voice.note as f32 - 69.0) / 12.0);

                // Oscillator 1
                let osc1_detune_factor = 2.0f32.powf(osc1_detune / 1200.0);
                let freq1 = base_freq * osc1_detune_factor;
                let phase_inc1 = freq1 / sample_rate;
                let osc1_sample = oscillator::generate_bandlimited(osc1_waveform, voice.phase1, phase_inc1);

                voice.phase1 += phase_inc1;
                if voice.phase1 >= 1.0 {
                    voice.phase1 -= 1.0;
                }

                // Mix oscillators
                let osc_sample = if osc2_enabled {
                    // Oscillator 2 with pitch offset and detune
                    let osc2_pitch_factor = 2.0f32.powf(osc2_pitch as f32 / 12.0);
                    let osc2_detune_factor = 2.0f32.powf(osc2_detune / 1200.0);
                    let freq2 = base_freq * osc2_pitch_factor * osc2_detune_factor;
                    let phase_inc2 = freq2 / sample_rate;
                    let osc2_sample = oscillator::generate_bandlimited(osc2_waveform, voice.phase2, phase_inc2);

                    voice.phase2 += phase_inc2;
                    if voice.phase2 >= 1.0 {
                        voice.phase2 -= 1.0;
                    }

                    // Mix: 0.0 = all osc1, 1.0 = all osc2
                    osc1_sample * (1.0 - osc_mix) + osc2_sample * osc_mix
                } else {
                    osc1_sample
                };

                // Process filter envelope and apply filter
                let filter_env_level = Self::process_filter_envelope(voice, &filter_env_params);

                // Calculate modulated cutoff based on filter envelope
                // Envelope modulates cutoff upward from base frequency
                // filter_env_amount controls how many octaves the envelope sweeps
                let octave_range = 4.0; // Sweep up to 4 octaves
                let env_mod = filter_env_level * filter_env_amount * octave_range;
                let modulated_cutoff = filter_base_cutoff * 2.0f32.powf(env_mod);

                // Calculate filter coefficients for modulated cutoff
                let filter_coeffs = if filter_enabled && filter_env_amount > 0.0 {
                    calculate_filter_coeffs(filter_type, modulated_cutoff, filter_resonance, sample_rate)
                } else if filter_enabled {
                    // Use base coefficients when no envelope modulation
                    calculate_filter_coeffs(filter_type, filter_base_cutoff, filter_resonance, sample_rate)
                } else {
                    (1.0, 0.0, 0.0, 0.0, 0.0) // Passthrough
                };

                // Apply filter (per-voice state for proper polyphony)
                let filtered_sample = Self::apply_filter(
                    osc_sample,
                    &mut voice.filter_state,
                    filter_enabled,
                    filter_coeffs,
                );

                // Process amplitude envelope
                let env_level = Self::process_envelope(voice, &env_params);

                // Add to mix with velocity scaling
                let velocity_scale = voice.velocity as f32 / 127.0;
                sample += filtered_sample * env_level * velocity_scale;
            }

            // Apply effects chain (distortion → delay → reverb)
            sample = state.effects.process(sample);

            // Apply master volume and soft clip
            sample *= volume;
            sample = Self::soft_clip(sample);

            // Output to all channels
            for ch in frame.iter_mut() {
                *ch = sample;
            }

            // Record sample if recording is enabled
            if state.recording_enabled && state.recording_buffer.len() < state.recording_max_samples {
                // Record stereo (duplicate mono for now)
                state.recording_buffer.push(sample);
                state.recording_buffer.push(sample);
            }

            voice_count_out.store(active_count, Ordering::Relaxed);
        }
    }

    /// Apply filter to a sample using pre-extracted coefficients
    #[inline]
    fn apply_filter(
        input: f32,
        state: &mut BiquadState,
        enabled: bool,
        coeffs: (f32, f32, f32, f32, f32),
    ) -> f32 {
        if !enabled {
            return input;
        }

        let (b0, b1, b2, a1, a2) = coeffs;

        let output = b0 * input
            + b1 * state.x1()
            + b2 * state.x2()
            - a1 * state.y1()
            - a2 * state.y2();

        // Update delay lines
        state.update(input, output);

        output
    }

    /// Process the filter envelope for a voice
    fn process_filter_envelope(voice: &mut Voice, params: &EnvelopeParams) -> f32 {
        match voice.filter_envelope {
            EnvelopeState::Idle => {
                voice.filter_envelope_level = 0.0;
            }
            EnvelopeState::Attack => {
                let attack_rate = 1.0 / (params.attack * params.sample_rate);
                voice.filter_envelope_level += attack_rate;
                if voice.filter_envelope_level >= 1.0 {
                    voice.filter_envelope_level = 1.0;
                    voice.filter_envelope = EnvelopeState::Decay;
                }
            }
            EnvelopeState::Decay => {
                let decay_rate = 1.0 / (params.decay * params.sample_rate);
                voice.filter_envelope_level -= decay_rate;
                if voice.filter_envelope_level <= params.sustain {
                    voice.filter_envelope_level = params.sustain;
                    voice.filter_envelope = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                voice.filter_envelope_level = params.sustain;
            }
            EnvelopeState::Release => {
                let release_rate = voice.filter_envelope_level / (params.release * params.sample_rate);
                voice.filter_envelope_level -= release_rate;
                if voice.filter_envelope_level <= 0.001 {
                    voice.filter_envelope_level = 0.0;
                    voice.filter_envelope = EnvelopeState::Idle;
                }
            }
        }

        voice.filter_envelope_level
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
        voice.phase1 = 0.0;
        voice.phase2 = 0.0;
        voice.envelope = EnvelopeState::Attack;
        voice.envelope_level = 0.0;
        voice.filter_envelope = EnvelopeState::Attack;
        voice.filter_envelope_level = 0.0;
        voice.active = true;
        voice.filter_state.reset(); // Reset filter to prevent clicks

        log::debug!("Note on: {} vel={} voice={}", note, velocity, voice_idx);
    }

    /// Trigger a note off
    pub fn note_off(&mut self, note: u8) {
        let mut state = self.state.write();

        for voice in state.voices.iter_mut() {
            if voice.active && voice.note == note && voice.envelope != EnvelopeState::Release {
                voice.envelope = EnvelopeState::Release;
                voice.filter_envelope = EnvelopeState::Release;
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
            voice.filter_envelope = EnvelopeState::Release;
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

    // === Oscillator 1 controls ===

    /// Set oscillator 1 waveform
    pub fn set_osc1_waveform(&mut self, waveform: Waveform) {
        self.state.write().osc1_waveform = waveform;
    }

    /// Get oscillator 1 waveform
    pub fn osc1_waveform(&self) -> Waveform {
        self.state.read().osc1_waveform
    }

    /// Cycle oscillator 1 to next waveform
    pub fn next_osc1_waveform(&mut self) {
        let mut state = self.state.write();
        state.osc1_waveform = state.osc1_waveform.next();
    }

    /// Cycle oscillator 1 to previous waveform
    pub fn prev_osc1_waveform(&mut self) {
        let mut state = self.state.write();
        state.osc1_waveform = state.osc1_waveform.prev();
    }

    /// Set oscillator 1 detune in cents
    pub fn set_osc1_detune(&mut self, cents: f32) {
        self.state.write().osc1_detune = cents.clamp(-100.0, 100.0);
    }

    /// Get oscillator 1 detune
    pub fn osc1_detune(&self) -> f32 {
        self.state.read().osc1_detune
    }

    // === Oscillator 2 controls ===

    /// Toggle oscillator 2
    pub fn toggle_osc2(&mut self) {
        let mut state = self.state.write();
        state.osc2_enabled = !state.osc2_enabled;
    }

    /// Set oscillator 2 enabled state
    pub fn set_osc2_enabled(&mut self, enabled: bool) {
        self.state.write().osc2_enabled = enabled;
    }

    /// Get oscillator 2 enabled state
    pub fn osc2_enabled(&self) -> bool {
        self.state.read().osc2_enabled
    }

    /// Set oscillator 2 waveform
    pub fn set_osc2_waveform(&mut self, waveform: Waveform) {
        self.state.write().osc2_waveform = waveform;
    }

    /// Get oscillator 2 waveform
    pub fn osc2_waveform(&self) -> Waveform {
        self.state.read().osc2_waveform
    }

    /// Cycle oscillator 2 to next waveform
    pub fn next_osc2_waveform(&mut self) {
        let mut state = self.state.write();
        state.osc2_waveform = state.osc2_waveform.next();
    }

    /// Set oscillator 2 detune in cents
    pub fn set_osc2_detune(&mut self, cents: f32) {
        self.state.write().osc2_detune = cents.clamp(-100.0, 100.0);
    }

    /// Get oscillator 2 detune
    pub fn osc2_detune(&self) -> f32 {
        self.state.read().osc2_detune
    }

    /// Set oscillator 2 pitch offset in semitones
    pub fn set_osc2_pitch(&mut self, semitones: i32) {
        self.state.write().osc2_pitch = semitones.clamp(-24, 24);
    }

    /// Get oscillator 2 pitch offset
    pub fn osc2_pitch(&self) -> i32 {
        self.state.read().osc2_pitch
    }

    /// Set oscillator mix (0.0 = all osc1, 1.0 = all osc2)
    pub fn set_osc_mix(&mut self, mix: f32) {
        self.state.write().osc_mix = mix.clamp(0.0, 1.0);
    }

    /// Get oscillator mix
    pub fn osc_mix(&self) -> f32 {
        self.state.read().osc_mix
    }

    // === Filter controls ===

    /// Set filter type
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.state.write().filter.set_type(filter_type);
    }

    /// Get filter type
    pub fn filter_type(&self) -> FilterType {
        self.state.read().filter.filter_type()
    }

    /// Cycle filter to next type
    pub fn next_filter_type(&mut self) {
        let mut state = self.state.write();
        let next_type = state.filter.filter_type().next();
        state.filter.set_type(next_type);
    }

    /// Cycle filter to previous type
    pub fn prev_filter_type(&mut self) {
        let mut state = self.state.write();
        let prev_type = state.filter.filter_type().prev();
        state.filter.set_type(prev_type);
    }

    /// Set filter cutoff frequency
    pub fn set_filter_cutoff(&mut self, freq: f32) {
        self.state.write().filter.set_cutoff(freq);
    }

    /// Get filter cutoff frequency
    pub fn filter_cutoff(&self) -> f32 {
        self.state.read().filter.cutoff()
    }

    /// Adjust filter cutoff by a factor (multiplicative)
    pub fn adjust_filter_cutoff(&mut self, factor: f32) {
        let mut state = self.state.write();
        let current = state.filter.cutoff();
        state.filter.set_cutoff(current * factor);
    }

    /// Set filter resonance (Q factor)
    pub fn set_filter_resonance(&mut self, q: f32) {
        self.state.write().filter.set_resonance(q);
    }

    /// Get filter resonance
    pub fn filter_resonance(&self) -> f32 {
        self.state.read().filter.resonance()
    }

    /// Adjust filter resonance
    pub fn adjust_filter_resonance(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.filter.resonance();
        state.filter.set_resonance(current + delta);
    }

    /// Toggle filter enabled state
    pub fn toggle_filter(&mut self) {
        let mut state = self.state.write();
        let enabled = state.filter.enabled();
        state.filter.set_enabled(!enabled);
    }

    /// Set filter enabled state
    pub fn set_filter_enabled(&mut self, enabled: bool) {
        self.state.write().filter.set_enabled(enabled);
    }

    /// Get filter enabled state
    pub fn filter_enabled(&self) -> bool {
        self.state.read().filter.enabled()
    }

    // === Filter envelope controls ===

    /// Get filter envelope amount (0.0 - 1.0)
    pub fn filter_env_amount(&self) -> f32 {
        self.state.read().filter_env_amount
    }

    /// Set filter envelope amount (0.0 - 1.0)
    pub fn set_filter_env_amount(&mut self, amount: f32) {
        self.state.write().filter_env_amount = amount.clamp(0.0, 1.0);
    }

    /// Adjust filter envelope amount
    pub fn adjust_filter_env_amount(&mut self, delta: f32) {
        let mut state = self.state.write();
        state.filter_env_amount = (state.filter_env_amount + delta).clamp(0.0, 1.0);
    }

    // === Effects controls ===

    // -- Distortion --

    /// Toggle distortion
    pub fn toggle_distortion(&mut self) {
        self.state.write().effects.distortion.toggle();
    }

    /// Set distortion enabled state
    pub fn set_distortion_enabled(&mut self, enabled: bool) {
        self.state.write().effects.distortion.set_enabled(enabled);
    }

    /// Check if distortion is enabled
    pub fn distortion_enabled(&self) -> bool {
        self.state.read().effects.distortion.enabled()
    }

    /// Cycle distortion type
    pub fn next_distortion_type(&mut self) {
        self.state.write().effects.distortion.next_type();
    }

    /// Set distortion type
    pub fn set_distortion_type(&mut self, dist_type: super::effects::DistortionType) {
        self.state.write().effects.distortion.set_type(dist_type);
    }

    /// Get distortion type name
    pub fn distortion_type_name(&self) -> &'static str {
        self.state.read().effects.distortion.dist_type().name()
    }

    /// Adjust distortion drive
    pub fn adjust_distortion_drive(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.effects.distortion.drive();
        state.effects.distortion.set_drive(current + delta);
    }

    /// Set distortion drive
    pub fn set_distortion_drive(&mut self, drive: f32) {
        self.state.write().effects.distortion.set_drive(drive);
    }

    /// Get distortion drive
    pub fn distortion_drive(&self) -> f32 {
        self.state.read().effects.distortion.drive()
    }

    // -- Delay --

    /// Toggle delay
    pub fn toggle_delay(&mut self) {
        self.state.write().effects.delay.toggle();
    }

    /// Set delay enabled state
    pub fn set_delay_enabled(&mut self, enabled: bool) {
        self.state.write().effects.delay.set_enabled(enabled);
    }

    /// Check if delay is enabled
    pub fn delay_enabled(&self) -> bool {
        self.state.read().effects.delay.enabled()
    }

    /// Adjust delay time
    pub fn adjust_delay_time(&mut self, delta_ms: f32) {
        let mut state = self.state.write();
        let sample_rate = state.sample_rate;
        let current = state.effects.delay.delay_ms(sample_rate);
        state.effects.delay.set_delay_ms(sample_rate, (current + delta_ms).clamp(10.0, 1000.0));
    }

    /// Set delay time in ms
    pub fn set_delay_time(&mut self, time_ms: f32) {
        let mut state = self.state.write();
        let sample_rate = state.sample_rate;
        state.effects.delay.set_delay_ms(sample_rate, time_ms.clamp(10.0, 1000.0));
    }

    /// Set delay feedback
    pub fn set_delay_feedback(&mut self, feedback: f32) {
        self.state.write().effects.delay.set_feedback(feedback);
    }

    /// Set delay mix
    pub fn set_delay_mix(&mut self, mix: f32) {
        self.state.write().effects.delay.set_mix(mix);
    }

    /// Get delay time in ms
    pub fn delay_time_ms(&self) -> f32 {
        let state = self.state.read();
        state.effects.delay.delay_ms(state.sample_rate)
    }

    /// Adjust delay feedback
    pub fn adjust_delay_feedback(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.effects.delay.feedback();
        state.effects.delay.set_feedback(current + delta);
    }

    /// Get delay feedback
    pub fn delay_feedback(&self) -> f32 {
        self.state.read().effects.delay.feedback()
    }

    /// Adjust delay mix
    pub fn adjust_delay_mix(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.effects.delay.mix();
        state.effects.delay.set_mix(current + delta);
    }

    /// Get delay mix
    pub fn delay_mix(&self) -> f32 {
        self.state.read().effects.delay.mix()
    }

    // -- Reverb --

    /// Toggle reverb
    pub fn toggle_reverb(&mut self) {
        self.state.write().effects.reverb.toggle();
    }

    /// Set reverb enabled state
    pub fn set_reverb_enabled(&mut self, enabled: bool) {
        self.state.write().effects.reverb.set_enabled(enabled);
    }

    /// Check if reverb is enabled
    pub fn reverb_enabled(&self) -> bool {
        self.state.read().effects.reverb.enabled()
    }

    /// Adjust reverb decay
    pub fn adjust_reverb_decay(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.effects.reverb.decay();
        state.effects.reverb.set_decay(current + delta);
    }

    /// Set reverb decay
    pub fn set_reverb_decay(&mut self, decay: f32) {
        self.state.write().effects.reverb.set_decay(decay);
    }

    /// Get reverb decay
    pub fn reverb_decay(&self) -> f32 {
        self.state.read().effects.reverb.decay()
    }

    /// Adjust reverb mix
    pub fn adjust_reverb_mix(&mut self, delta: f32) {
        let mut state = self.state.write();
        let current = state.effects.reverb.mix();
        state.effects.reverb.set_mix(current + delta);
    }

    /// Set reverb mix
    pub fn set_reverb_mix(&mut self, mix: f32) {
        self.state.write().effects.reverb.set_mix(mix);
    }

    /// Get reverb mix
    pub fn reverb_mix(&self) -> f32 {
        self.state.read().effects.reverb.mix()
    }

    // === Recording controls ===

    /// Start recording
    pub fn start_recording(&mut self) {
        let mut state = self.state.write();
        state.recording_buffer.clear();
        state.recording_enabled = true;
        log::info!("Recording started");
    }

    /// Stop recording and return the buffer
    pub fn stop_recording(&mut self) -> Vec<f32> {
        let mut state = self.state.write();
        state.recording_enabled = false;
        let buffer = std::mem::take(&mut state.recording_buffer);
        log::info!("Recording stopped: {} samples", buffer.len());
        buffer
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.state.read().recording_enabled
    }

    /// Get recording duration in seconds
    pub fn recording_duration(&self) -> f32 {
        let state = self.state.read();
        state.recording_buffer.len() as f32 / (state.sample_rate * 2.0)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> f32 {
        self.state.read().sample_rate
    }

    /// Stop the audio engine
    pub fn stop(&mut self) {
        self.panic();
    }
}
