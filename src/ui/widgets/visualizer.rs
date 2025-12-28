//! Audio visualization widgets (waveform, spectrum, combined)

use crate::app::VizMode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Widget},
};
use rustfft::{num_complex::Complex, FftPlanner};

/// Vertical block characters for drawing (8 levels)
const VBLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Audio visualizer widget
pub struct Visualizer<'a> {
    /// Audio samples to visualize
    samples: &'a [f32],
    /// Sample rate for spectrum calculations
    sample_rate: f32,
    /// Visualization mode
    mode: VizMode,
    /// Block for border/title
    block: Option<Block<'a>>,
}

impl<'a> Visualizer<'a> {
    pub fn new(samples: &'a [f32], sample_rate: f32, mode: VizMode) -> Self {
        Self {
            samples,
            sample_rate,
            mode,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Downsample waveform to fit width
    fn get_waveform_points(&self, width: usize) -> Vec<f32> {
        if self.samples.is_empty() || width == 0 {
            return vec![0.0; width];
        }

        let samples_per_point = self.samples.len() / width;
        if samples_per_point == 0 {
            return self.samples.iter().copied().collect();
        }

        // For each point, get the min/max in that range (shows peaks better)
        (0..width)
            .map(|i| {
                let start = i * samples_per_point;
                let end = (start + samples_per_point).min(self.samples.len());
                let chunk = &self.samples[start..end];

                // Use RMS for smoother display
                let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
                // Also track peak for more responsive display
                let peak = chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                // Blend RMS and peak
                rms * 0.6 + peak * 0.4
            })
            .collect()
    }

    /// Compute FFT spectrum (returns magnitude bins)
    fn get_spectrum(&self, num_bins: usize) -> Vec<f32> {
        if self.samples.is_empty() || num_bins == 0 {
            return vec![0.0; num_bins];
        }

        // Use power of 2 for FFT
        let fft_size = self.samples.len().min(2048);

        // Apply Hann window and convert to complex
        let mut buffer: Vec<Complex<f32>> = self.samples[..fft_size]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos());
                Complex::new(s * window, 0.0)
            })
            .collect();

        // Perform FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        fft.process(&mut buffer);

        // Convert to magnitudes (only use first half - positive frequencies)
        let nyquist = fft_size / 2;
        let bins_per_output = nyquist / num_bins;

        if bins_per_output == 0 {
            return vec![0.0; num_bins];
        }

        // Group FFT bins into display bins
        (0..num_bins)
            .map(|i| {
                let start = i * bins_per_output;
                let end = (start + bins_per_output).min(nyquist);

                // Average magnitude in this range
                let sum: f32 = buffer[start..end]
                    .iter()
                    .map(|c| c.norm())
                    .sum();

                // Convert to dB-like scale for better visualization
                let avg = sum / (end - start) as f32;
                // Logarithmic scaling with some gain
                (avg.max(0.0001).log10() + 4.0).max(0.0) / 4.0
            })
            .collect()
    }

    /// Draw waveform in the given area
    fn draw_waveform(&self, buf: &mut Buffer, area: Rect) {
        let points = self.get_waveform_points(area.width as usize);
        let height = area.height as f32;
        let mid_y = area.y + area.height / 2;

        for (x, &amplitude) in points.iter().enumerate() {
            let x_pos = area.x + x as u16;
            if x_pos >= area.x + area.width {
                break;
            }

            // Map amplitude to vertical position (centered)
            let scaled = (amplitude * height * 0.9).min(height / 2.0);

            // Draw above and below center line
            let top = (mid_y as f32 - scaled).max(area.y as f32) as u16;
            let bottom = (mid_y as f32 + scaled).min((area.y + area.height - 1) as f32) as u16;

            for y in top..=bottom {
                if y >= area.y && y < area.y + area.height {
                    buf[(x_pos, y)].set_char('│').set_fg(Color::Cyan);
                }
            }
        }

        // Draw center line
        for x in area.x..area.x + area.width {
            let cell = &mut buf[(x, mid_y)];
            if cell.symbol() == " " {
                cell.set_char('─').set_fg(Color::DarkGray);
            }
        }
    }

    /// Draw spectrum analyzer in the given area
    fn draw_spectrum(&self, buf: &mut Buffer, area: Rect) {
        let spectrum = self.get_spectrum(area.width as usize);
        let height = area.height as f32;

        for (x, &magnitude) in spectrum.iter().enumerate() {
            let x_pos = area.x + x as u16;
            if x_pos >= area.x + area.width {
                break;
            }

            // Map magnitude to bar height
            let bar_height = (magnitude * height).min(height);
            let full_blocks = bar_height as u16;
            let partial = ((bar_height - full_blocks as f32) * 8.0) as usize;

            // Color based on frequency (left = bass = red, right = treble = cyan)
            let color = match x * 4 / area.width as usize {
                0 => Color::Red,
                1 => Color::Yellow,
                2 => Color::Green,
                _ => Color::Cyan,
            };

            // Draw from bottom up
            for h in 0..full_blocks {
                let y = area.y + area.height - 1 - h;
                if y >= area.y {
                    buf[(x_pos, y)].set_char('█').set_fg(color);
                }
            }

            // Draw partial block at top
            if partial > 0 && full_blocks < area.height {
                let y = area.y + area.height - 1 - full_blocks;
                if y >= area.y {
                    buf[(x_pos, y)].set_char(VBLOCKS[partial]).set_fg(color);
                }
            }
        }
    }
}

impl Widget for Visualizer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(block) = &self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.width < 2 || inner.height < 2 {
            return;
        }

        match self.mode {
            VizMode::Off => {
                // Draw placeholder text
                let text = "Press Home to enable visualizer";
                let x = inner.x + (inner.width.saturating_sub(text.len() as u16)) / 2;
                let y = inner.y + inner.height / 2;
                for (i, c) in text.chars().enumerate() {
                    if x + (i as u16) < inner.x + inner.width {
                        buf[(x + i as u16, y)].set_char(c).set_fg(Color::DarkGray);
                    }
                }
            }
            VizMode::Waveform => {
                self.draw_waveform(buf, inner);
            }
            VizMode::Spectrum => {
                self.draw_spectrum(buf, inner);
            }
            VizMode::Combined => {
                // Split area: top half waveform, bottom half spectrum
                let wave_area = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: inner.height / 2,
                };
                let spec_area = Rect {
                    x: inner.x,
                    y: inner.y + inner.height / 2,
                    width: inner.width,
                    height: inner.height - inner.height / 2,
                };
                self.draw_waveform(buf, wave_area);
                self.draw_spectrum(buf, spec_area);
            }
        }
    }
}
