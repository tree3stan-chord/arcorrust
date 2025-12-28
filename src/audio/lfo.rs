//! Low Frequency Oscillator for modulation

use std::collections::HashSet;
use std::f32::consts::TAU;

/// LFO waveform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LFOWaveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    SampleHold,
}

impl LFOWaveform {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            LFOWaveform::Sine => "Sine",
            LFOWaveform::Triangle => "Tri",
            LFOWaveform::Saw => "Saw",
            LFOWaveform::Square => "Sqr",
            LFOWaveform::SampleHold => "S&H",
        }
    }

    /// Cycle to next waveform
    pub fn next(&self) -> Self {
        match self {
            LFOWaveform::Sine => LFOWaveform::Triangle,
            LFOWaveform::Triangle => LFOWaveform::Saw,
            LFOWaveform::Saw => LFOWaveform::Square,
            LFOWaveform::Square => LFOWaveform::SampleHold,
            LFOWaveform::SampleHold => LFOWaveform::Sine,
        }
    }

    /// Cycle to previous waveform
    pub fn prev(&self) -> Self {
        match self {
            LFOWaveform::Sine => LFOWaveform::SampleHold,
            LFOWaveform::Triangle => LFOWaveform::Sine,
            LFOWaveform::Saw => LFOWaveform::Triangle,
            LFOWaveform::Square => LFOWaveform::Saw,
            LFOWaveform::SampleHold => LFOWaveform::Square,
        }
    }
}

/// LFO modulation destinations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LFODestination {
    Pitch,
    Volume,
    FilterCutoff,
    PulseWidth,
    Osc2Pitch,
    Pan,
}

impl LFODestination {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            LFODestination::Pitch => "Pitch",
            LFODestination::Volume => "Volume",
            LFODestination::FilterCutoff => "Filter",
            LFODestination::PulseWidth => "PWM",
            LFODestination::Osc2Pitch => "Osc2",
            LFODestination::Pan => "Pan",
        }
    }

    /// Get short name for UI
    pub fn short_name(&self) -> &'static str {
        match self {
            LFODestination::Pitch => "P",
            LFODestination::Volume => "V",
            LFODestination::FilterCutoff => "F",
            LFODestination::PulseWidth => "W",
            LFODestination::Osc2Pitch => "O",
            LFODestination::Pan => "A",
        }
    }

    /// All destinations for iteration
    pub fn all() -> &'static [LFODestination] {
        &[
            LFODestination::Pitch,
            LFODestination::Volume,
            LFODestination::FilterCutoff,
            LFODestination::PulseWidth,
            LFODestination::Osc2Pitch,
            LFODestination::Pan,
        ]
    }
}

/// Low Frequency Oscillator
#[derive(Debug, Clone)]
pub struct LFO {
    /// LFO enabled
    pub enabled: bool,
    /// Rate in Hz (0.1 - 20 Hz)
    pub rate: f32,
    /// Waveform type
    pub waveform: LFOWaveform,
    /// Modulation depth (0.0 - 1.0)
    pub depth: f32,
    /// Current phase (0.0 - 1.0)
    phase: f32,
    /// Active modulation destinations
    pub destinations: HashSet<LFODestination>,
    /// Held sample for S&H
    sample_hold_value: f32,
    /// Previous phase for S&H trigger detection
    prev_phase: f32,
}

impl Default for LFO {
    fn default() -> Self {
        Self {
            enabled: false,
            rate: 2.0,
            waveform: LFOWaveform::default(),
            depth: 0.5,
            phase: 0.0,
            destinations: HashSet::new(),
            sample_hold_value: 0.0,
            prev_phase: 0.0,
        }
    }
}

impl LFO {
    /// Create a new LFO with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the LFO and return the current value (-1.0 to 1.0)
    pub fn tick(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        // Advance phase
        self.prev_phase = self.phase;
        self.phase += self.rate / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        // Generate waveform
        let raw_value = match self.waveform {
            LFOWaveform::Sine => (self.phase * TAU).sin(),
            LFOWaveform::Triangle => {
                if self.phase < 0.5 {
                    4.0 * self.phase - 1.0
                } else {
                    3.0 - 4.0 * self.phase
                }
            }
            LFOWaveform::Saw => 2.0 * self.phase - 1.0,
            LFOWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LFOWaveform::SampleHold => {
                // Update held sample on phase wrap
                if self.phase < self.prev_phase {
                    self.sample_hold_value = fastrand::f32() * 2.0 - 1.0;
                }
                self.sample_hold_value
            }
        };

        raw_value * self.depth
    }

    /// Get current LFO value without advancing (for UI display)
    pub fn current_value(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let raw_value = match self.waveform {
            LFOWaveform::Sine => (self.phase * TAU).sin(),
            LFOWaveform::Triangle => {
                if self.phase < 0.5 {
                    4.0 * self.phase - 1.0
                } else {
                    3.0 - 4.0 * self.phase
                }
            }
            LFOWaveform::Saw => 2.0 * self.phase - 1.0,
            LFOWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LFOWaveform::SampleHold => self.sample_hold_value,
        };

        raw_value * self.depth
    }

    /// Reset the LFO phase
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_phase = 0.0;
        self.sample_hold_value = 0.0;
    }

    /// Toggle a destination on/off
    pub fn toggle_destination(&mut self, dest: LFODestination) {
        if self.destinations.contains(&dest) {
            self.destinations.remove(&dest);
        } else {
            self.destinations.insert(dest);
        }
    }

    /// Check if a destination is active
    pub fn has_destination(&self, dest: LFODestination) -> bool {
        self.destinations.contains(&dest)
    }

    /// Set rate with clamping
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.1, 20.0);
    }

    /// Adjust rate by delta
    pub fn adjust_rate(&mut self, delta: f32) {
        self.set_rate(self.rate + delta);
    }

    /// Set depth with clamping
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    /// Adjust depth by delta
    pub fn adjust_depth(&mut self, delta: f32) {
        self.set_depth(self.depth + delta);
    }

    /// Cycle to next waveform
    pub fn next_waveform(&mut self) {
        self.waveform = self.waveform.next();
    }

    /// Cycle to previous waveform
    pub fn prev_waveform(&mut self) {
        self.waveform = self.waveform.prev();
    }

    /// Calculate pitch modulation multiplier
    /// Returns a frequency multiplier (e.g., 1.0 for no change, 2.0 for octave up)
    pub fn pitch_mod(&self, lfo_value: f32, semitones: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::Pitch) {
            return 1.0;
        }
        // Convert semitones to frequency ratio
        // lfo_value is -depth to +depth, map to -semitones to +semitones
        2.0_f32.powf(lfo_value * semitones / 12.0)
    }

    /// Calculate filter cutoff modulation multiplier
    /// Returns a frequency multiplier
    pub fn filter_mod(&self, lfo_value: f32, octaves: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::FilterCutoff) {
            return 1.0;
        }
        2.0_f32.powf(lfo_value * octaves)
    }

    /// Calculate volume modulation (tremolo)
    /// Returns amplitude multiplier (0.0 to 1.0)
    pub fn volume_mod(&self, lfo_value: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::Volume) {
            return 1.0;
        }
        // Map -1..1 to 0..1, but with depth control
        // At depth 1.0, goes from 0 to 1
        // At depth 0.5, goes from 0.5 to 1
        1.0 - ((-lfo_value + 1.0) / 2.0)
    }

    /// Calculate pan modulation
    /// Returns pan value (-1.0 left to 1.0 right)
    pub fn pan_mod(&self, lfo_value: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::Pan) {
            return 0.0;
        }
        lfo_value
    }

    /// Calculate pulse width modulation offset
    /// Returns offset to add to base pulse width
    pub fn pwm_mod(&self, lfo_value: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::PulseWidth) {
            return 0.0;
        }
        // Map to reasonable PWM range (about +/- 0.4 at full depth)
        lfo_value * 0.4
    }

    /// Calculate oscillator 2 pitch modulation
    pub fn osc2_pitch_mod(&self, lfo_value: f32, semitones: f32) -> f32 {
        if !self.enabled || !self.has_destination(LFODestination::Osc2Pitch) {
            return 1.0;
        }
        2.0_f32.powf(lfo_value * semitones / 12.0)
    }
}
