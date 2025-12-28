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

    pub fn feedback(&self) -> f32 {
        self.feedback
    }

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

    pub fn clear(&mut self) {
        for buf in &mut self.comb_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.allpass_buffers {
            buf.fill(0.0);
        }
    }
}

/// Combined effects chain
#[derive(Clone)]
pub struct EffectsChain {
    pub distortion: Distortion,
    pub delay: Delay,
    pub reverb: Reverb,
}

impl EffectsChain {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            distortion: Distortion::new(),
            delay: Delay::new(sample_rate, 1000.0), // Max 1 second delay
            reverb: Reverb::new(sample_rate),
        }
    }

    /// Process a sample through the effects chain
    /// Order: Distortion → Delay → Reverb
    pub fn process(&mut self, input: f32) -> f32 {
        let distorted = self.distortion.process(input);
        let delayed = self.delay.process(distorted);
        self.reverb.process(delayed)
    }

    pub fn clear(&mut self) {
        self.delay.clear();
        self.reverb.clear();
    }
}
