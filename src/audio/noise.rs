//! Noise generator for percussion and texture

/// Noise types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoiseType {
    #[default]
    White,
    Pink,
    Brown,
}

impl NoiseType {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            NoiseType::White => "White",
            NoiseType::Pink => "Pink",
            NoiseType::Brown => "Brown",
        }
    }

    /// Cycle to next type
    pub fn next(&self) -> Self {
        match self {
            NoiseType::White => NoiseType::Pink,
            NoiseType::Pink => NoiseType::Brown,
            NoiseType::Brown => NoiseType::White,
        }
    }
}

/// Noise generator with multiple noise types
#[derive(Debug, Clone)]
pub struct NoiseGenerator {
    /// Noise type
    pub noise_type: NoiseType,
    /// Output level (0.0 - 1.0)
    pub level: f32,
    /// Whether noise is enabled
    pub enabled: bool,
    /// Pink noise state (Voss-McCartney algorithm)
    pink_rows: [f32; 16],
    pink_index: usize,
    pink_running_sum: f32,
    /// Brown noise state (integration of white noise)
    brown_value: f32,
}

impl Default for NoiseGenerator {
    fn default() -> Self {
        Self {
            noise_type: NoiseType::default(),
            level: 0.5,
            enabled: false,
            pink_rows: [0.0; 16],
            pink_index: 0,
            pink_running_sum: 0.0,
            brown_value: 0.0,
        }
    }
}

impl NoiseGenerator {
    /// Create a new noise generator
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a noise sample
    pub fn tick(&mut self) -> f32 {
        if !self.enabled || self.level <= 0.0 {
            return 0.0;
        }

        let sample = match self.noise_type {
            NoiseType::White => self.generate_white(),
            NoiseType::Pink => self.generate_pink(),
            NoiseType::Brown => self.generate_brown(),
        };

        sample * self.level
    }

    /// Generate white noise (-1.0 to 1.0)
    #[inline]
    fn generate_white(&self) -> f32 {
        fastrand::f32() * 2.0 - 1.0
    }

    /// Generate pink noise using Voss-McCartney algorithm
    /// Pink noise has equal energy per octave (-3dB/octave)
    fn generate_pink(&mut self) -> f32 {
        // Voss-McCartney algorithm: use counter bits to determine which
        // row to update, maintaining a running sum
        let white = fastrand::f32() * 2.0 - 1.0;

        // Find which row to update based on trailing zeros of index
        let k = self.pink_index.trailing_zeros() as usize;
        self.pink_index = self.pink_index.wrapping_add(1);

        if k < 16 {
            // Remove old value from sum
            self.pink_running_sum -= self.pink_rows[k];
            // Generate new value
            let new_value = fastrand::f32() * 2.0 - 1.0;
            self.pink_rows[k] = new_value;
            // Add new value to sum
            self.pink_running_sum += new_value;
        }

        // Combine running sum with white noise and normalize
        // The factor 0.0625 (1/16) normalizes the sum of 16 rows
        (self.pink_running_sum * 0.0625 + white * 0.25) * 0.8
    }

    /// Generate brown/red noise (integration of white noise)
    /// Brown noise has -6dB/octave rolloff
    fn generate_brown(&mut self) -> f32 {
        // Integrate white noise with leak
        let white = fastrand::f32() * 2.0 - 1.0;
        // Leak factor prevents DC buildup and keeps values bounded
        self.brown_value = (self.brown_value + white * 0.02) * 0.998;
        // Scale to reasonable range
        (self.brown_value * 3.5).clamp(-1.0, 1.0)
    }

    /// Reset noise generator state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.pink_rows = [0.0; 16];
        self.pink_index = 0;
        self.pink_running_sum = 0.0;
        self.brown_value = 0.0;
    }

    /// Set noise level (0.0 - 1.0)
    pub fn set_level(&mut self, level: f32) {
        self.level = level.clamp(0.0, 1.0);
    }

    /// Adjust noise level
    #[allow(dead_code)]
    pub fn adjust_level(&mut self, delta: f32) {
        self.set_level(self.level + delta);
    }

    /// Cycle to next noise type
    pub fn next_type(&mut self) {
        self.noise_type = self.noise_type.next();
    }
}
