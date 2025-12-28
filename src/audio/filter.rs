//! Biquad filter implementation for LP/HP/BP/Notch filtering

use std::f32::consts::PI;

/// Calculate biquad filter coefficients for a given cutoff frequency
/// This allows for dynamic modulation of the filter cutoff (e.g., via envelope)
/// Returns (b0, b1, b2, a1, a2) normalized coefficients
pub fn calculate_filter_coeffs(
    filter_type: FilterType,
    cutoff: f32,
    resonance: f32,
    sample_rate: f32,
) -> (f32, f32, f32, f32, f32) {
    // Clamp cutoff to valid range
    let nyquist = sample_rate * 0.5;
    let cutoff = cutoff.clamp(20.0, nyquist - 100.0);

    let omega = 2.0 * PI * cutoff / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * resonance);

    let (b0, b1, b2, a0, a1, a2) = match filter_type {
        FilterType::LowPass => {
            let b1 = 1.0 - cos_omega;
            let b0 = b1 / 2.0;
            let b2 = b0;
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * cos_omega;
            let a2 = 1.0 - alpha;
            (b0, b1, b2, a0, a1, a2)
        }
        FilterType::HighPass => {
            let b1 = -(1.0 + cos_omega);
            let b0 = (1.0 + cos_omega) / 2.0;
            let b2 = b0;
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * cos_omega;
            let a2 = 1.0 - alpha;
            (b0, b1, b2, a0, a1, a2)
        }
        FilterType::BandPass => {
            let b0 = alpha;
            let b1 = 0.0;
            let b2 = -alpha;
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * cos_omega;
            let a2 = 1.0 - alpha;
            (b0, b1, b2, a0, a1, a2)
        }
        FilterType::Notch => {
            let b0 = 1.0;
            let b1 = -2.0 * cos_omega;
            let b2 = 1.0;
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * cos_omega;
            let a2 = 1.0 - alpha;
            (b0, b1, b2, a0, a1, a2)
        }
    };

    // Normalize by a0
    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

/// Filter type selection
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum FilterType {
    #[default]
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

impl FilterType {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            FilterType::LowPass => "LP",
            FilterType::HighPass => "HP",
            FilterType::BandPass => "BP",
            FilterType::Notch => "Notch",
        }
    }

    /// Cycle to next filter type
    pub fn next(&self) -> Self {
        match self {
            FilterType::LowPass => FilterType::HighPass,
            FilterType::HighPass => FilterType::BandPass,
            FilterType::BandPass => FilterType::Notch,
            FilterType::Notch => FilterType::LowPass,
        }
    }

    /// Cycle to previous filter type
    #[allow(dead_code)]
    pub fn prev(&self) -> Self {
        match self {
            FilterType::LowPass => FilterType::Notch,
            FilterType::HighPass => FilterType::LowPass,
            FilterType::BandPass => FilterType::HighPass,
            FilterType::Notch => FilterType::BandPass,
        }
    }
}

/// Biquad filter coefficients (used internally by BiquadFilter)
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Biquad filter state (per-voice)
#[derive(Clone, Copy, Debug, Default)]
pub struct BiquadState {
    /// Input delay line
    x1: f32,
    x2: f32,
    /// Output delay line
    y1: f32,
    y2: f32,
}

impl BiquadState {
    /// Reset filter state (call on note on to prevent clicks)
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Get x1 (previous input)
    #[inline]
    pub fn x1(&self) -> f32 {
        self.x1
    }

    /// Get x2 (input from 2 samples ago)
    #[inline]
    pub fn x2(&self) -> f32 {
        self.x2
    }

    /// Get y1 (previous output)
    #[inline]
    pub fn y1(&self) -> f32 {
        self.y1
    }

    /// Get y2 (output from 2 samples ago)
    #[inline]
    pub fn y2(&self) -> f32 {
        self.y2
    }

    /// Update delay lines with new input and output
    #[inline]
    pub fn update(&mut self, input: f32, output: f32) {
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
    }
}

/// Biquad filter with coefficient calculation
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    /// Filter type
    filter_type: FilterType,
    /// Cutoff frequency in Hz
    cutoff: f32,
    /// Resonance (Q factor), 0.5 to 20.0
    resonance: f32,
    /// Sample rate
    sample_rate: f32,
    /// Filter enabled
    enabled: bool,
    /// Calculated coefficients
    coeffs: Coefficients,
}

impl BiquadFilter {
    /// Create a new biquad filter
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = Self {
            filter_type: FilterType::LowPass,
            cutoff: 8000.0,
            resonance: 0.707, // Butterworth Q
            sample_rate,
            enabled: true,
            coeffs: Coefficients::default(),
        };
        filter.calculate_coefficients();
        filter
    }

    /// Set filter type and recalculate
    pub fn set_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.calculate_coefficients();
    }

    /// Get filter type
    pub fn filter_type(&self) -> FilterType {
        self.filter_type
    }

    /// Set cutoff frequency (20 Hz to Nyquist)
    pub fn set_cutoff(&mut self, freq: f32) {
        let nyquist = self.sample_rate * 0.5;
        self.cutoff = freq.clamp(20.0, nyquist - 100.0);
        self.calculate_coefficients();
    }

    /// Get cutoff frequency
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }

    /// Set resonance (Q factor, 0.5 to 20.0)
    pub fn set_resonance(&mut self, q: f32) {
        self.resonance = q.clamp(0.5, 20.0);
        self.calculate_coefficients();
    }

    /// Get resonance
    pub fn resonance(&self) -> f32 {
        self.resonance
    }

    /// Enable/disable filter
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if filter is enabled
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Update sample rate (call if audio config changes)
    #[allow(dead_code)]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.calculate_coefficients();
    }

    /// Calculate coefficients based on filter type
    /// Using Robert Bristow-Johnson's Audio EQ Cookbook formulas
    fn calculate_coefficients(&mut self) {
        let omega = 2.0 * PI * self.cutoff / self.sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * self.resonance);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_omega;
                let b0 = b1 / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b1 = -(1.0 + cos_omega);
                let b0 = (1.0 + cos_omega) / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                // Constant skirt gain, peak gain = Q
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalize by a0
        self.coeffs = Coefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        };
    }

    /// Process a single sample through the filter
    /// Uses Direct Form II Transposed for better numerical stability
    #[inline]
    #[allow(dead_code)]
    pub fn process(&self, input: f32, state: &mut BiquadState) -> f32 {
        if !self.enabled {
            return input;
        }

        let output = self.coeffs.b0 * input
            + self.coeffs.b1 * state.x1
            + self.coeffs.b2 * state.x2
            - self.coeffs.a1 * state.y1
            - self.coeffs.a2 * state.y2;

        // Update delay lines
        state.x2 = state.x1;
        state.x1 = input;
        state.y2 = state.y1;
        state.y1 = output;

        output
    }

    /// Get coefficients for external processing (e.g., modulation)
    #[allow(dead_code)]
    pub fn get_coefficients(&self) -> (f32, f32, f32, f32, f32) {
        (
            self.coeffs.b0,
            self.coeffs.b1,
            self.coeffs.b2,
            self.coeffs.a1,
            self.coeffs.a2,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_type_cycle() {
        let mut ft = FilterType::LowPass;
        ft = ft.next();
        assert_eq!(ft, FilterType::HighPass);
        ft = ft.next();
        assert_eq!(ft, FilterType::BandPass);
        ft = ft.next();
        assert_eq!(ft, FilterType::Notch);
        ft = ft.next();
        assert_eq!(ft, FilterType::LowPass);
    }

    #[test]
    fn test_filter_passthrough() {
        let mut filter = BiquadFilter::new(44100.0);
        filter.set_enabled(false);
        let mut state = BiquadState::default();

        // Disabled filter should pass through unchanged
        assert_eq!(filter.process(0.5, &mut state), 0.5);
        assert_eq!(filter.process(-0.3, &mut state), -0.3);
    }

    #[test]
    fn test_lowpass_attenuates_high_freq() {
        let mut filter = BiquadFilter::new(44100.0);
        filter.set_type(FilterType::LowPass);
        filter.set_cutoff(100.0); // Very low cutoff
        filter.set_resonance(0.707);

        let mut state = BiquadState::default();

        // Generate a high frequency signal (5000 Hz)
        let freq = 5000.0;
        let sample_rate = 44100.0;
        let mut sum_input = 0.0f32;
        let mut sum_output = 0.0f32;

        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let input = (2.0 * PI * freq * t).sin();
            let output = filter.process(input, &mut state);
            sum_input += input.abs();
            sum_output += output.abs();
        }

        // Output should be significantly attenuated
        assert!(sum_output < sum_input * 0.1);
    }
}
