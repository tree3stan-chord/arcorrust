//! Oscillator waveform generation

use std::f32::consts::PI;

/// Oscillator waveform types
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Waveform {
    #[default]
    Sine,
    Sawtooth,
    Square,
    Triangle,
    Pulse { width: f32 }, // width 0.0-1.0, 0.5 = square
}

impl Waveform {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Sawtooth => "Saw",
            Waveform::Square => "Square",
            Waveform::Triangle => "Tri",
            Waveform::Pulse { .. } => "Pulse",
        }
    }

    /// Cycle through waveforms
    pub fn next(&self) -> Self {
        match self {
            Waveform::Sine => Waveform::Sawtooth,
            Waveform::Sawtooth => Waveform::Square,
            Waveform::Square => Waveform::Triangle,
            Waveform::Triangle => Waveform::Sine,
            Waveform::Pulse { .. } => Waveform::Sine,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Waveform::Sine => Waveform::Triangle,
            Waveform::Sawtooth => Waveform::Sine,
            Waveform::Square => Waveform::Sawtooth,
            Waveform::Triangle => Waveform::Square,
            Waveform::Pulse { .. } => Waveform::Triangle,
        }
    }
}

/// Generate a sample for the given waveform at the given phase
///
/// # Arguments
/// * `waveform` - The type of waveform to generate
/// * `phase` - Phase position (0.0 to 1.0)
///
/// # Returns
/// Sample value (-1.0 to 1.0)
pub fn generate(waveform: Waveform, phase: f32) -> f32 {
    match waveform {
        Waveform::Sine => generate_sine(phase),
        Waveform::Sawtooth => generate_saw(phase),
        Waveform::Square => generate_square(phase),
        Waveform::Triangle => generate_triangle(phase),
        Waveform::Pulse { width } => generate_pulse(phase, width),
    }
}

/// Generate a band-limited sample using polyBLEP for alias reduction
///
/// # Arguments
/// * `waveform` - The type of waveform to generate
/// * `phase` - Phase position (0.0 to 1.0)
/// * `phase_inc` - Phase increment per sample (freq / sample_rate)
///
/// # Returns
/// Sample value (-1.0 to 1.0)
pub fn generate_bandlimited(waveform: Waveform, phase: f32, phase_inc: f32) -> f32 {
    match waveform {
        Waveform::Sine => generate_sine(phase),
        Waveform::Sawtooth => generate_saw_blep(phase, phase_inc),
        Waveform::Square => generate_square_blep(phase, phase_inc),
        Waveform::Triangle => generate_triangle(phase), // Triangle is naturally band-limited
        Waveform::Pulse { width } => generate_pulse_blep(phase, phase_inc, width),
    }
}

// === Basic waveform generators ===

#[inline]
fn generate_sine(phase: f32) -> f32 {
    (phase * 2.0 * PI).sin()
}

#[inline]
fn generate_saw(phase: f32) -> f32 {
    2.0 * phase - 1.0
}

#[inline]
fn generate_square(phase: f32) -> f32 {
    if phase < 0.5 { 1.0 } else { -1.0 }
}

#[inline]
fn generate_triangle(phase: f32) -> f32 {
    let t = phase * 4.0;
    if phase < 0.25 {
        t
    } else if phase < 0.75 {
        2.0 - t
    } else {
        t - 4.0
    }
}

#[inline]
fn generate_pulse(phase: f32, width: f32) -> f32 {
    if phase < width { 1.0 } else { -1.0 }
}

// === PolyBLEP anti-aliasing ===

/// PolyBLEP (polynomial band-limited step) correction
/// Reduces aliasing at discontinuities
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        // 0 <= t < dt
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        // 1-dt < t <= 1
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[inline]
fn generate_saw_blep(phase: f32, phase_inc: f32) -> f32 {
    let mut sample = generate_saw(phase);
    sample -= poly_blep(phase, phase_inc);
    sample
}

#[inline]
fn generate_square_blep(phase: f32, phase_inc: f32) -> f32 {
    let mut sample = generate_square(phase);
    sample += poly_blep(phase, phase_inc);
    sample -= poly_blep((phase + 0.5) % 1.0, phase_inc);
    sample
}

#[inline]
fn generate_pulse_blep(phase: f32, phase_inc: f32, width: f32) -> f32 {
    let mut sample = generate_pulse(phase, width);
    sample += poly_blep(phase, phase_inc);
    sample -= poly_blep((phase + (1.0 - width)) % 1.0, phase_inc);
    sample
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_range() {
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let sample = generate_sine(phase);
            assert!(sample >= -1.0 && sample <= 1.0);
        }
    }

    #[test]
    fn test_saw_range() {
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let sample = generate_saw(phase);
            assert!(sample >= -1.0 && sample <= 1.0);
        }
    }

    #[test]
    fn test_square_values() {
        assert_eq!(generate_square(0.0), 1.0);
        assert_eq!(generate_square(0.25), 1.0);
        assert_eq!(generate_square(0.5), -1.0);
        assert_eq!(generate_square(0.75), -1.0);
    }

    #[test]
    fn test_triangle_range() {
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let sample = generate_triangle(phase);
            assert!(sample >= -1.0 && sample <= 1.0, "phase={} sample={}", phase, sample);
        }
    }
}
