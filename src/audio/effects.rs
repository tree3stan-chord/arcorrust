//! Audio effects - delay, distortion, and reverb

/// Delay effect with feedback
#[derive(Clone)]
pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    delay_samples: usize,
    feedback: f32,
    mix: f32,
    enabled: bool,
}

impl Delay {
    pub fn new(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_samples = (sample_rate * max_delay_ms / 1000.0) as usize;
        Self {
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            delay_samples: (sample_rate * 250.0 / 1000.0) as usize, // 250ms default
            feedback: 0.4,
            mix: 0.3,
            enabled: false,
        }
    }

    pub fn set_delay_ms(&mut self, sample_rate: f32, delay_ms: f32) {
        let samples = (sample_rate * delay_ms / 1000.0) as usize;
        self.delay_samples = samples.min(self.buffer.len() - 1);
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.95); // Prevent runaway feedback
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn delay_ms(&self, sample_rate: f32) -> f32 {
        self.delay_samples as f32 * 1000.0 / sample_rate
    }

    #[allow(dead_code)]
    pub fn feedback(&self) -> f32 {
        self.feedback
    }

    #[allow(dead_code)]
    pub fn mix(&self) -> f32 {
        self.mix
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // Read from delay line
        let read_pos = if self.write_pos >= self.delay_samples {
            self.write_pos - self.delay_samples
        } else {
            self.buffer.len() - (self.delay_samples - self.write_pos)
        };

        let delayed = self.buffer[read_pos];

        // Write to delay line with feedback
        self.buffer[self.write_pos] = input + delayed * self.feedback;

        // Advance write position
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        // Mix dry and wet
        input * (1.0 - self.mix) + delayed * self.mix
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// Distortion effect with multiple modes
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistortionType {
    SoftClip,
    HardClip,
    Tanh,
    Foldback,
}

impl DistortionType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SoftClip => "Soft",
            Self::HardClip => "Hard",
            Self::Tanh => "Tanh",
            Self::Foldback => "Fold",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::SoftClip => Self::HardClip,
            Self::HardClip => Self::Tanh,
            Self::Tanh => Self::Foldback,
            Self::Foldback => Self::SoftClip,
        }
    }
}

#[derive(Clone)]
pub struct Distortion {
    drive: f32,        // 1.0 - 10.0
    output_gain: f32,  // Compensate for increased volume
    dist_type: DistortionType,
    enabled: bool,
}

impl Distortion {
    pub fn new() -> Self {
        Self {
            drive: 2.0,
            output_gain: 0.5,
            dist_type: DistortionType::SoftClip,
            enabled: false,
        }
    }

    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(1.0, 10.0);
        // Auto-adjust output gain based on drive
        self.output_gain = 1.0 / self.drive.sqrt();
    }

    pub fn drive(&self) -> f32 {
        self.drive
    }

    pub fn set_type(&mut self, dist_type: DistortionType) {
        self.dist_type = dist_type;
    }

    pub fn dist_type(&self) -> DistortionType {
        self.dist_type
    }

    pub fn next_type(&mut self) {
        self.dist_type = self.dist_type.next();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process(&self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        let driven = input * self.drive;

        let distorted = match self.dist_type {
            DistortionType::SoftClip => {
                // Soft clipping using polynomial
                if driven > 1.0 {
                    2.0 / 3.0
                } else if driven < -1.0 {
                    -2.0 / 3.0
                } else {
                    driven - (driven * driven * driven) / 3.0
                }
            }
            DistortionType::HardClip => {
                driven.clamp(-1.0, 1.0)
            }
            DistortionType::Tanh => {
                driven.tanh()
            }
            DistortionType::Foldback => {
                // Foldback distortion - wraps signal back on itself
                let threshold = 1.0;
                let mut x = driven;
                while x > threshold || x < -threshold {
                    if x > threshold {
                        x = 2.0 * threshold - x;
                    } else if x < -threshold {
                        x = -2.0 * threshold - x;
                    }
                }
                x
            }
        };

        distorted * self.output_gain
    }
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple Schroeder reverb
#[derive(Clone)]
pub struct Reverb {
    // Comb filters (parallel)
    comb_buffers: [Vec<f32>; 4],
    comb_positions: [usize; 4],
    comb_feedback: f32,

    // All-pass filters (series)
    allpass_buffers: [Vec<f32>; 2],
    allpass_positions: [usize; 2],

    mix: f32,
    enabled: bool,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        // Comb filter delay times in ms (tuned for room reverb)
        let comb_delays_ms = [29.7, 37.1, 41.1, 43.7];
        let comb_buffers: [Vec<f32>; 4] = comb_delays_ms
            .map(|ms| vec![0.0; (sample_rate * ms / 1000.0) as usize]);

        // All-pass filter delay times
        let allpass_delays_ms = [5.0, 1.7];
        let allpass_buffers: [Vec<f32>; 2] = allpass_delays_ms
            .map(|ms| vec![0.0; (sample_rate * ms / 1000.0) as usize]);

        Self {
            comb_buffers,
            comb_positions: [0; 4],
            comb_feedback: 0.84,
            allpass_buffers,
            allpass_positions: [0; 2],
            mix: 0.3,
            enabled: false,
        }
    }

    pub fn set_decay(&mut self, decay: f32) {
        // decay 0.0-1.0 maps to feedback 0.7-0.95
        self.comb_feedback = 0.7 + decay.clamp(0.0, 1.0) * 0.25;
    }

    pub fn decay(&self) -> f32 {
        (self.comb_feedback - 0.7) / 0.25
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn mix(&self) -> f32 {
        self.mix
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // Process parallel comb filters
        let mut comb_out = 0.0;
        for i in 0..4 {
            let buf = &mut self.comb_buffers[i];
            let pos = self.comb_positions[i];

            let delayed = buf[pos];
            buf[pos] = input + delayed * self.comb_feedback;

            self.comb_positions[i] = (pos + 1) % buf.len();
            comb_out += delayed;
        }
        comb_out *= 0.25; // Average the comb outputs

        // Process series all-pass filters
        let mut allpass_out = comb_out;
        for i in 0..2 {
            let buf = &mut self.allpass_buffers[i];
            let pos = self.allpass_positions[i];

            let delayed = buf[pos];
            let feedback = 0.5;

            buf[pos] = allpass_out + delayed * feedback;
            allpass_out = delayed - allpass_out * feedback;

            self.allpass_positions[i] = (pos + 1) % buf.len();
        }

        // Mix dry and wet
        input * (1.0 - self.mix) + allpass_out * self.mix
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        for buf in &mut self.comb_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.allpass_buffers {
            buf.fill(0.0);
        }
    }
}

/// Ring modulator effect - multiplies signal with carrier oscillator
#[derive(Clone)]
pub struct RingMod {
    /// Carrier frequency in Hz (20 - 2000)
    carrier_freq: f32,
    /// Carrier phase (0.0 - 1.0)
    carrier_phase: f32,
    /// Wet/dry mix (0.0 - 1.0)
    mix: f32,
    /// Sample rate
    sample_rate: f32,
    /// Enabled state
    enabled: bool,
}

impl RingMod {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            carrier_freq: 200.0,    // 200 Hz default
            carrier_phase: 0.0,
            mix: 0.5,               // 50/50 wet/dry
            sample_rate,
            enabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn set_carrier_freq(&mut self, freq: f32) {
        self.carrier_freq = freq.clamp(20.0, 2000.0);
    }

    pub fn carrier_freq(&self) -> f32 {
        self.carrier_freq
    }

    pub fn adjust_carrier_freq(&mut self, delta: f32) {
        self.carrier_freq = (self.carrier_freq + delta).clamp(20.0, 2000.0);
    }

    #[allow(dead_code)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn mix(&self) -> f32 {
        self.mix
    }

    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // Generate carrier sine wave
        let carrier = (self.carrier_phase * std::f32::consts::TAU).sin();

        // Advance carrier phase
        self.carrier_phase += self.carrier_freq / self.sample_rate;
        if self.carrier_phase >= 1.0 {
            self.carrier_phase -= 1.0;
        }

        // Ring modulation: multiply input by carrier
        let ring = input * carrier;

        // Mix dry and wet
        input * (1.0 - self.mix) + ring * self.mix
    }
}

impl Default for RingMod {
    fn default() -> Self {
        Self::new(44100.0)
    }
}

/// Phaser effect - creates sweeping notches using allpass filters
#[derive(Clone)]
pub struct Phaser {
    /// LFO rate in Hz (0.1 - 5.0)
    rate: f32,
    /// Modulation depth (0.0 - 1.0)
    depth: f32,
    /// Feedback amount (-0.95 to 0.95)
    feedback: f32,
    /// Number of allpass stages (4, 6, 8, or 12)
    stages: u8,
    /// Wet/dry mix (0.0 - 1.0)
    mix: f32,
    /// Allpass filter states (y1 for each stage)
    allpass_y1: Vec<f32>,
    /// LFO phase
    lfo_phase: f32,
    /// Sample rate
    sample_rate: f32,
    /// Last output for feedback
    feedback_sample: f32,
    /// Enabled state
    enabled: bool,
}

impl Phaser {
    /// Minimum and maximum cutoff frequencies for the allpass filters
    const MIN_FREQ: f32 = 200.0;
    const MAX_FREQ: f32 = 4000.0;

    pub fn new(sample_rate: f32) -> Self {
        Self {
            rate: 0.5,           // 0.5 Hz default
            depth: 0.7,          // 70% depth
            feedback: 0.5,       // 50% feedback
            stages: 6,           // 6 stages default
            mix: 0.5,            // 50/50 wet/dry
            allpass_y1: vec![0.0; 12], // Pre-allocate for max stages
            lfo_phase: 0.0,
            sample_rate,
            feedback_sample: 0.0,
            enabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.1, 5.0);
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    #[allow(dead_code)]
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    #[allow(dead_code)]
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(-0.95, 0.95);
    }

    #[allow(dead_code)]
    pub fn feedback(&self) -> f32 {
        self.feedback
    }

    #[allow(dead_code)]
    pub fn set_stages(&mut self, stages: u8) {
        // Only allow 4, 6, 8, or 12
        self.stages = match stages {
            0..=5 => 4,
            6..=7 => 6,
            8..=10 => 8,
            _ => 12,
        };
    }

    pub fn stages(&self) -> u8 {
        self.stages
    }

    #[allow(dead_code)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    #[allow(dead_code)]
    pub fn mix(&self) -> f32 {
        self.mix
    }

    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// First-order allpass filter
    /// coefficient = (tan(pi * f / sr) - 1) / (tan(pi * f / sr) + 1)
    #[inline]
    fn allpass(&mut self, input: f32, stage: usize, freq: f32) -> f32 {
        // Compute allpass coefficient
        let w = std::f32::consts::PI * freq / self.sample_rate;
        let tan_w = w.tan();
        let coeff = (tan_w - 1.0) / (tan_w + 1.0);

        // First-order allpass filter
        let y1 = self.allpass_y1[stage];
        let output = coeff * (input - y1) + y1;

        self.allpass_y1[stage] = output;
        output
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // LFO generates modulated cutoff frequency
        let lfo = (self.lfo_phase * std::f32::consts::TAU).sin();
        // Map LFO to frequency range with depth control
        let freq_range = Self::MAX_FREQ - Self::MIN_FREQ;
        let center_freq = (Self::MAX_FREQ + Self::MIN_FREQ) / 2.0;
        let mod_freq = center_freq + lfo * freq_range * 0.5 * self.depth;

        // Add feedback
        let input_with_feedback = input + self.feedback_sample * self.feedback;

        // Process through allpass chain
        let mut signal = input_with_feedback;
        for stage in 0..(self.stages as usize) {
            signal = self.allpass(signal, stage, mod_freq);
        }

        // Store feedback sample
        self.feedback_sample = signal;

        // Advance LFO
        self.lfo_phase += self.rate / self.sample_rate;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }

        // Mix dry and wet (wet inverted for classic phaser sound)
        input * (1.0 - self.mix) + signal * self.mix
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.allpass_y1.fill(0.0);
        self.feedback_sample = 0.0;
    }
}

impl Default for Phaser {
    fn default() -> Self {
        Self::new(44100.0)
    }
}

/// Chorus effect - creates a rich, detuned sound using modulated delay lines
#[derive(Clone)]
pub struct Chorus {
    /// LFO rate in Hz (0.1 - 5.0)
    rate: f32,
    /// Modulation depth in ms (0.5 - 5.0)
    depth: f32,
    /// Number of chorus voices (1-4)
    voices: u8,
    /// Wet/dry mix (0.0 - 1.0)
    mix: f32,
    /// Delay lines for each voice
    delay_lines: Vec<Vec<f32>>,
    /// Write positions for each delay line
    write_positions: Vec<usize>,
    /// LFO phases for each voice
    lfo_phases: Vec<f32>,
    /// Sample rate
    sample_rate: f32,
    /// Base delay in samples
    base_delay_samples: usize,
    /// Enabled state
    enabled: bool,
}

impl Chorus {
    /// Base delay time in ms (center of modulation)
    const BASE_DELAY_MS: f32 = 20.0;
    /// Maximum delay time in ms (for buffer sizing)
    const MAX_DELAY_MS: f32 = 50.0;

    pub fn new(sample_rate: f32) -> Self {
        let max_samples = (sample_rate * Self::MAX_DELAY_MS / 1000.0) as usize;
        let base_samples = (sample_rate * Self::BASE_DELAY_MS / 1000.0) as usize;

        // Create 4 delay lines (max voices)
        let delay_lines = vec![vec![0.0; max_samples]; 4];
        let write_positions = vec![0; 4];
        // Spread LFO phases for each voice
        let lfo_phases = vec![0.0, 0.25, 0.5, 0.75];

        Self {
            rate: 1.0,           // 1 Hz default
            depth: 2.0,          // 2ms modulation
            voices: 2,           // 2 voices default
            mix: 0.5,            // 50/50 wet/dry
            delay_lines,
            write_positions,
            lfo_phases,
            sample_rate,
            base_delay_samples: base_samples,
            enabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.1, 5.0);
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    #[allow(dead_code)]
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.5, 5.0);
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    #[allow(dead_code)]
    pub fn set_voices(&mut self, voices: u8) {
        self.voices = voices.clamp(1, 4);
    }

    pub fn voices(&self) -> u8 {
        self.voices
    }

    #[allow(dead_code)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    #[allow(dead_code)]
    pub fn mix(&self) -> f32 {
        self.mix
    }

    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        let depth_samples = self.depth * self.sample_rate / 1000.0;
        let phase_inc = self.rate / self.sample_rate;
        let mut wet = 0.0;

        for i in 0..(self.voices as usize) {
            let buffer = &mut self.delay_lines[i];
            let write_pos = self.write_positions[i];
            let buffer_len = buffer.len();

            // Write input to delay line
            buffer[write_pos] = input;

            // Calculate modulated read position using sine LFO
            let lfo = (self.lfo_phases[i] * std::f32::consts::TAU).sin();
            let mod_delay = self.base_delay_samples as f32 + lfo * depth_samples;

            // Read from delay line with linear interpolation
            let read_pos_float = write_pos as f32 - mod_delay;
            let read_pos_float = if read_pos_float < 0.0 {
                read_pos_float + buffer_len as f32
            } else {
                read_pos_float
            };

            let read_pos_int = read_pos_float.floor() as usize % buffer_len;
            let frac = read_pos_float.fract();
            let next_pos = (read_pos_int + 1) % buffer_len;

            let delayed = buffer[read_pos_int] * (1.0 - frac) + buffer[next_pos] * frac;
            wet += delayed;

            // Advance write position
            self.write_positions[i] = (write_pos + 1) % buffer_len;

            // Advance LFO phase (with slight rate variation per voice for richer sound)
            let voice_rate_factor = 1.0 + (i as f32 * 0.05);
            self.lfo_phases[i] += phase_inc * voice_rate_factor;
            if self.lfo_phases[i] >= 1.0 {
                self.lfo_phases[i] -= 1.0;
            }
        }

        // Average the wet signal
        wet /= self.voices as f32;

        // Mix dry and wet
        input * (1.0 - self.mix) + wet * self.mix
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        for buffer in &mut self.delay_lines {
            buffer.fill(0.0);
        }
    }
}

impl Default for Chorus {
    fn default() -> Self {
        Self::new(44100.0)
    }
}

/// Bitcrusher effect - reduces bit depth and sample rate
#[derive(Clone)]
pub struct Bitcrusher {
    /// Bit depth (1-16, lower = more crunchy)
    bit_depth: u8,
    /// Sample rate divider (1-64, higher = lower sample rate)
    sample_rate_div: u8,
    /// Wet/dry mix (0.0 - 1.0)
    mix: f32,
    /// Held sample for sample rate reduction
    held_sample: f32,
    /// Counter for sample rate reduction
    sample_counter: u8,
    /// Enabled state
    enabled: bool,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Self {
            bit_depth: 8,        // 8-bit by default (classic game sound)
            sample_rate_div: 1,  // No rate reduction by default
            mix: 1.0,            // Full wet by default
            held_sample: 0.0,
            sample_counter: 0,
            enabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn set_bit_depth(&mut self, bits: u8) {
        self.bit_depth = bits.clamp(1, 16);
    }

    pub fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    pub fn adjust_bit_depth(&mut self, delta: i8) {
        let new_depth = (self.bit_depth as i16 + delta as i16).clamp(1, 16) as u8;
        self.bit_depth = new_depth;
    }

    #[allow(dead_code)]
    pub fn set_sample_rate_div(&mut self, div: u8) {
        self.sample_rate_div = div.clamp(1, 64);
    }

    pub fn sample_rate_div(&self) -> u8 {
        self.sample_rate_div
    }

    pub fn adjust_sample_rate_div(&mut self, delta: i8) {
        let new_div = (self.sample_rate_div as i16 + delta as i16).clamp(1, 64) as u8;
        self.sample_rate_div = new_div;
    }

    #[allow(dead_code)]
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    #[allow(dead_code)]
    pub fn mix(&self) -> f32 {
        self.mix
    }

    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // Sample rate reduction - only update held sample at reduced rate
        self.sample_counter += 1;
        if self.sample_counter >= self.sample_rate_div {
            self.sample_counter = 0;

            // Bit depth reduction
            // Convert to integer levels, round, then back to float
            let levels = (1u32 << self.bit_depth) as f32;
            // Map [-1, 1] to [0, levels], round, then back to [-1, 1]
            let normalized = (input + 1.0) * 0.5; // [0, 1]
            let quantized = (normalized * levels).round() / levels;
            self.held_sample = quantized * 2.0 - 1.0; // back to [-1, 1]
        }

        // Mix dry and crushed
        input * (1.0 - self.mix) + self.held_sample * self.mix
    }
}

impl Default for Bitcrusher {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined effects chain
#[derive(Clone)]
pub struct EffectsChain {
    pub ring_mod: RingMod,
    pub distortion: Distortion,
    pub chorus: Chorus,
    pub phaser: Phaser,
    pub delay: Delay,
    pub reverb: Reverb,
    pub bitcrusher: Bitcrusher,
}

impl EffectsChain {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            ring_mod: RingMod::new(sample_rate),
            distortion: Distortion::new(),
            chorus: Chorus::new(sample_rate),
            phaser: Phaser::new(sample_rate),
            delay: Delay::new(sample_rate, 1000.0), // Max 1 second delay
            reverb: Reverb::new(sample_rate),
            bitcrusher: Bitcrusher::new(),
        }
    }

    /// Process a sample through the effects chain
    /// Order: RingMod → Distortion → Chorus → Phaser → Delay → Reverb → Bitcrusher
    pub fn process(&mut self, input: f32) -> f32 {
        let ring_modded = self.ring_mod.process(input);
        let distorted = self.distortion.process(ring_modded);
        let chorused = self.chorus.process(distorted);
        let phased = self.phaser.process(chorused);
        let delayed = self.delay.process(phased);
        let reverbed = self.reverb.process(delayed);
        self.bitcrusher.process(reverbed)
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.chorus.clear();
        self.phaser.clear();
        self.delay.clear();
        self.reverb.clear();
    }
}
