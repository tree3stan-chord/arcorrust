//! WAV audio recorder - save audio buffers to WAV files

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Save audio samples to a WAV file
pub fn save_wav(samples: &[f32], sample_rate: u32, path: &Path) -> Result<()> {
    let file = File::create(path)
        .context("Failed to create WAV file")?;
    let mut writer = BufWriter::new(file);

    let num_channels: u16 = 2; // Stereo
    let bits_per_sample: u16 = 16;
    let data_size = samples.len() * 2; // 2 bytes per 16-bit sample
    let file_size = 36 + data_size as u32;

    // Write RIFF header
    writer.write_all(b"RIFF")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    // Write fmt chunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // Chunk size
    writer.write_all(&1u16.to_le_bytes())?; // Audio format (PCM)
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    writer.write_all(&byte_rate.to_le_bytes())?;
    let block_align = num_channels * bits_per_sample / 8;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;

    // Write data chunk
    writer.write_all(b"data")?;
    writer.write_all(&(data_size as u32).to_le_bytes())?;

    // Convert f32 samples to i16 and write
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_all(&i16_sample.to_le_bytes())?;
    }

    writer.flush()
        .context("Failed to flush WAV file")?;

    log::info!("Saved WAV: {:?} ({} samples, {:.1}s)",
        path,
        samples.len() / 2,
        samples.len() as f32 / (sample_rate as f32 * 2.0)
    );

    Ok(())
}

/// Generate a unique filename for recording
pub fn generate_filename() -> PathBuf {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    PathBuf::from(format!("recording_{}.wav", timestamp))
}

/// Get the recordings directory (creates if needed)
pub fn recordings_dir() -> Result<PathBuf> {
    let dir = dirs::audio_dir()
        .map(|p| p.join("arcorrust"))
        .unwrap_or_else(|| PathBuf::from("recordings"));

    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .context("Failed to create recordings directory")?;
    }

    Ok(dir)
}
