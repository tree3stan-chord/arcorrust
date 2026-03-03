//! Preset management - save and load synthesizer settings

use crate::audio::{DistortionType, FilterType, Waveform};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Complete synthesizer preset
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    /// Preset name
    pub name: String,
    /// Preset author (optional)
    #[serde(default)]
    pub author: String,
    /// Preset description (optional)
    #[serde(default)]
    pub description: String,

    // Oscillator settings
    pub osc1_waveform: WaveformSer,
    pub osc1_detune: f32,
    pub osc2_enabled: bool,
    pub osc2_waveform: WaveformSer,
    pub osc2_detune: f32,
    pub osc2_pitch: i32,
    pub osc_mix: f32,

    // Filter settings
    pub filter_enabled: bool,
    pub filter_type: FilterTypeSer,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_env_amount: f32,

    // Amplitude envelope
    pub amp_attack: f32,
    pub amp_decay: f32,
    pub amp_sustain: f32,
    pub amp_release: f32,

    // Filter envelope
    pub filter_attack: f32,
    pub filter_decay: f32,
    pub filter_sustain: f32,
    pub filter_release: f32,

    // Effects
    pub distortion_enabled: bool,
    pub distortion_type: DistortionTypeSer,
    pub distortion_drive: f32,

    pub delay_enabled: bool,
    pub delay_time_ms: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,

    pub reverb_enabled: bool,
    pub reverb_decay: f32,
    pub reverb_mix: f32,

    // Master
    pub volume: f32,
}

/// Serializable waveform type
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WaveformSer {
    Sine,
    Triangle,
    Sawtooth,
    Square,
}

impl From<Waveform> for WaveformSer {
    fn from(w: Waveform) -> Self {
        match w {
            Waveform::Sine => Self::Sine,
            Waveform::Triangle => Self::Triangle,
            Waveform::Sawtooth => Self::Sawtooth,
            Waveform::Square => Self::Square,
            Waveform::Pulse { .. } => Self::Square, // Map pulse to square
        }
    }
}

impl From<WaveformSer> for Waveform {
    fn from(w: WaveformSer) -> Self {
        match w {
            WaveformSer::Sine => Waveform::Sine,
            WaveformSer::Triangle => Waveform::Triangle,
            WaveformSer::Sawtooth => Waveform::Sawtooth,
            WaveformSer::Square => Waveform::Square,
        }
    }
}

/// Serializable filter type
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FilterTypeSer {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

impl From<FilterType> for FilterTypeSer {
    fn from(f: FilterType) -> Self {
        match f {
            FilterType::LowPass => Self::Lowpass,
            FilterType::HighPass => Self::Highpass,
            FilterType::BandPass => Self::Bandpass,
            FilterType::Notch => Self::Notch,
        }
    }
}

impl From<FilterTypeSer> for FilterType {
    fn from(f: FilterTypeSer) -> Self {
        match f {
            FilterTypeSer::Lowpass => FilterType::LowPass,
            FilterTypeSer::Highpass => FilterType::HighPass,
            FilterTypeSer::Bandpass => FilterType::BandPass,
            FilterTypeSer::Notch => FilterType::Notch,
        }
    }
}

/// Serializable distortion type
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DistortionTypeSer {
    SoftClip,
    HardClip,
    Tanh,
    Foldback,
}

impl From<DistortionType> for DistortionTypeSer {
    fn from(d: DistortionType) -> Self {
        match d {
            DistortionType::SoftClip => Self::SoftClip,
            DistortionType::HardClip => Self::HardClip,
            DistortionType::Tanh => Self::Tanh,
            DistortionType::Foldback => Self::Foldback,
        }
    }
}

impl From<DistortionTypeSer> for DistortionType {
    fn from(d: DistortionTypeSer) -> Self {
        match d {
            DistortionTypeSer::SoftClip => DistortionType::SoftClip,
            DistortionTypeSer::HardClip => DistortionType::HardClip,
            DistortionTypeSer::Tanh => DistortionType::Tanh,
            DistortionTypeSer::Foldback => DistortionType::Foldback,
        }
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "Init".to_string(),
            author: String::new(),
            description: "Default initialization preset".to_string(),

            osc1_waveform: WaveformSer::Sawtooth,
            osc1_detune: 0.0,
            osc2_enabled: false,
            osc2_waveform: WaveformSer::Square,
            osc2_detune: 7.0,
            osc2_pitch: 0,
            osc_mix: 0.5,

            filter_enabled: true,
            filter_type: FilterTypeSer::Lowpass,
            filter_cutoff: 2000.0,
            filter_resonance: 1.0,
            filter_env_amount: 0.5,

            amp_attack: 0.01,
            amp_decay: 0.1,
            amp_sustain: 0.7,
            amp_release: 0.3,

            filter_attack: 0.001,
            filter_decay: 0.2,
            filter_sustain: 0.0,
            filter_release: 0.2,

            distortion_enabled: false,
            distortion_type: DistortionTypeSer::SoftClip,
            distortion_drive: 2.0,

            delay_enabled: false,
            delay_time_ms: 250.0,
            delay_feedback: 0.4,
            delay_mix: 0.3,

            reverb_enabled: false,
            reverb_decay: 0.5,
            reverb_mix: 0.3,

            volume: 0.8,
        }
    }
}

impl Preset {
    /// Create a new preset with default values
    #[allow(dead_code)]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Save preset to a JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize preset")?;
        fs::write(path, json)
            .context("Failed to write preset file")?;
        Ok(())
    }

    /// Load preset from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path)
            .context("Failed to read preset file")?;
        let preset: Preset = serde_json::from_str(&json)
            .context("Failed to parse preset file")?;
        Ok(preset)
    }
}

/// Preset manager - handles preset library
pub struct PresetManager {
    /// Directory containing presets
    preset_dir: PathBuf,
    /// List of available presets
    presets: Vec<PresetInfo>,
    /// Currently selected preset index
    current_index: usize,
}

/// Basic preset info for listing
#[derive(Clone, Debug)]
pub struct PresetInfo {
    pub name: String,
    pub path: PathBuf,
}

impl PresetManager {
    /// Create a new preset manager
    pub fn new() -> Result<Self> {
        // Use ~/.config/arcorrust/presets or ./presets
        let preset_dir = dirs::config_dir()
            .map(|p| p.join("arcorrust").join("presets"))
            .unwrap_or_else(|| PathBuf::from("presets"));

        // Create directory if it doesn't exist
        if !preset_dir.exists() {
            fs::create_dir_all(&preset_dir)
                .context("Failed to create presets directory")?;

            // Create some factory presets
            Self::create_factory_presets(&preset_dir)?;
        }

        let mut manager = Self {
            preset_dir,
            presets: Vec::new(),
            current_index: 0,
        };

        manager.scan_presets()?;
        Ok(manager)
    }

    /// Scan preset directory for available presets
    pub fn scan_presets(&mut self) -> Result<()> {
        self.presets.clear();

        if let Ok(entries) = fs::read_dir(&self.preset_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(preset) = Preset::load(&path) {
                        self.presets.push(PresetInfo {
                            name: preset.name,
                            path,
                        });
                    }
                }
            }
        }

        // Sort by name
        self.presets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    /// Get list of available presets
    pub fn list(&self) -> &[PresetInfo] {
        &self.presets
    }

    /// Get current preset index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get current preset name
    pub fn current_name(&self) -> &str {
        self.presets
            .get(self.current_index)
            .map(|p| p.name.as_str())
            .unwrap_or("None")
    }

    /// Load current preset
    pub fn load_current(&self) -> Result<Preset> {
        let info = self.presets
            .get(self.current_index)
            .context("No preset selected")?;
        Preset::load(&info.path)
    }

    /// Select next preset
    pub fn next(&mut self) {
        if !self.presets.is_empty() {
            self.current_index = (self.current_index + 1) % self.presets.len();
        }
    }

    /// Select previous preset
    pub fn prev(&mut self) {
        if !self.presets.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.presets.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Save a preset
    #[allow(dead_code)]
    pub fn save_preset(&mut self, preset: &Preset) -> Result<()> {
        let filename = format!("{}.json", preset.name.to_lowercase().replace(' ', "_"));
        let path = self.preset_dir.join(filename);
        preset.save(&path)?;
        self.scan_presets()?;
        Ok(())
    }

    /// Get preset directory path
    #[allow(dead_code)]
    pub fn preset_dir(&self) -> &Path {
        &self.preset_dir
    }

    /// Create factory presets
    fn create_factory_presets(dir: &Path) -> Result<()> {
        // Classic saw bass
        let bass = Preset {
            name: "Classic Bass".to_string(),
            description: "Deep saw bass with filter sweep".to_string(),
            osc1_waveform: WaveformSer::Sawtooth,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Sawtooth,
            osc2_detune: 10.0,
            osc2_pitch: -12,
            osc_mix: 0.4,
            filter_enabled: true,
            filter_cutoff: 400.0,
            filter_resonance: 3.0,
            filter_env_amount: 0.8,
            filter_attack: 0.001,
            filter_decay: 0.3,
            filter_sustain: 0.2,
            filter_release: 0.2,
            amp_attack: 0.005,
            amp_decay: 0.2,
            amp_sustain: 0.8,
            amp_release: 0.3,
            ..Default::default()
        };
        bass.save(&dir.join("classic_bass.json"))?;

        // Soft pad
        let pad = Preset {
            name: "Soft Pad".to_string(),
            description: "Warm evolving pad sound".to_string(),
            osc1_waveform: WaveformSer::Triangle,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Sawtooth,
            osc2_detune: 5.0,
            osc_mix: 0.3,
            filter_enabled: true,
            filter_cutoff: 1500.0,
            filter_resonance: 1.5,
            filter_env_amount: 0.3,
            amp_attack: 0.5,
            amp_decay: 0.3,
            amp_sustain: 0.8,
            amp_release: 1.0,
            reverb_enabled: true,
            reverb_mix: 0.4,
            ..Default::default()
        };
        pad.save(&dir.join("soft_pad.json"))?;

        // Plucky lead
        let lead = Preset {
            name: "Plucky Lead".to_string(),
            description: "Sharp attack lead with delay".to_string(),
            osc1_waveform: WaveformSer::Square,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Sawtooth,
            osc2_pitch: 12,
            osc_mix: 0.5,
            filter_enabled: true,
            filter_cutoff: 3000.0,
            filter_resonance: 2.0,
            filter_env_amount: 0.6,
            filter_decay: 0.15,
            amp_attack: 0.001,
            amp_decay: 0.1,
            amp_sustain: 0.5,
            amp_release: 0.2,
            delay_enabled: true,
            delay_time_ms: 375.0,
            delay_feedback: 0.35,
            delay_mix: 0.25,
            ..Default::default()
        };
        lead.save(&dir.join("plucky_lead.json"))?;

        // Dirty bass
        let dirty = Preset {
            name: "Dirty Bass".to_string(),
            description: "Distorted aggressive bass".to_string(),
            osc1_waveform: WaveformSer::Sawtooth,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Square,
            osc2_pitch: 0,
            osc2_detune: 15.0,
            osc_mix: 0.5,
            filter_enabled: true,
            filter_cutoff: 800.0,
            filter_resonance: 4.0,
            filter_env_amount: 0.7,
            distortion_enabled: true,
            distortion_type: DistortionTypeSer::Tanh,
            distortion_drive: 4.0,
            amp_attack: 0.001,
            amp_decay: 0.15,
            amp_sustain: 0.7,
            amp_release: 0.2,
            ..Default::default()
        };
        dirty.save(&dir.join("dirty_bass.json"))?;

        // Ambient texture
        let ambient = Preset {
            name: "Ambient Texture".to_string(),
            description: "Ethereal ambient sound".to_string(),
            osc1_waveform: WaveformSer::Sine,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Triangle,
            osc2_pitch: 7,
            osc2_detune: 3.0,
            osc_mix: 0.4,
            filter_enabled: true,
            filter_type: FilterTypeSer::Lowpass,
            filter_cutoff: 2000.0,
            filter_resonance: 1.0,
            amp_attack: 1.0,
            amp_decay: 0.5,
            amp_sustain: 0.6,
            amp_release: 2.0,
            delay_enabled: true,
            delay_time_ms: 500.0,
            delay_feedback: 0.5,
            delay_mix: 0.3,
            reverb_enabled: true,
            reverb_decay: 0.8,
            reverb_mix: 0.5,
            ..Default::default()
        };
        ambient.save(&dir.join("ambient_texture.json"))?;

        // Brass stab - punchy dual-saw brass with filter bite
        let brass = Preset {
            name: "Brass Stab".to_string(),
            description: "Thick detuned brass with filter punch".to_string(),
            osc1_waveform: WaveformSer::Sawtooth,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Sawtooth,
            osc2_detune: 12.0,
            osc2_pitch: 0,
            osc_mix: 0.5,
            filter_enabled: true,
            filter_type: FilterTypeSer::Lowpass,
            filter_cutoff: 1200.0,
            filter_resonance: 2.5,
            filter_env_amount: 0.7,
            filter_attack: 0.01,
            filter_decay: 0.25,
            filter_sustain: 0.4,
            filter_release: 0.15,
            amp_attack: 0.008,
            amp_decay: 0.15,
            amp_sustain: 0.85,
            amp_release: 0.15,
            distortion_enabled: true,
            distortion_type: DistortionTypeSer::Tanh,
            distortion_drive: 1.5,
            ..Default::default()
        };
        brass.save(&dir.join("brass_stab.json"))?;

        // Organ tone - full harmonic drawbar organ, direct and unwavering
        let organ = Preset {
            name: "Organ Tone".to_string(),
            description: "Warm full-body organ with immediate response".to_string(),
            osc1_waveform: WaveformSer::Square,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Square,
            osc2_detune: 0.0,
            osc2_pitch: 12,
            osc_mix: 0.35,
            filter_enabled: true,
            filter_type: FilterTypeSer::Lowpass,
            filter_cutoff: 3500.0,
            filter_resonance: 0.5,
            filter_env_amount: 0.0,
            amp_attack: 0.003,
            amp_decay: 0.05,
            amp_sustain: 1.0,
            amp_release: 0.08,
            distortion_enabled: true,
            distortion_type: DistortionTypeSer::SoftClip,
            distortion_drive: 1.8,
            reverb_enabled: true,
            reverb_decay: 0.3,
            reverb_mix: 0.15,
            ..Default::default()
        };
        organ.save(&dir.join("organ_tone.json"))?;

        // Thick lead - heavy detuned saw+square for a fat center-stage lead
        let thick_lead = Preset {
            name: "Thick Lead".to_string(),
            description: "Heavy saturated lead with wide detune".to_string(),
            osc1_waveform: WaveformSer::Sawtooth,
            osc2_enabled: true,
            osc2_waveform: WaveformSer::Square,
            osc2_detune: 18.0,
            osc2_pitch: 0,
            osc_mix: 0.45,
            filter_enabled: true,
            filter_type: FilterTypeSer::Lowpass,
            filter_cutoff: 2400.0,
            filter_resonance: 2.0,
            filter_env_amount: 0.4,
            filter_attack: 0.005,
            filter_decay: 0.2,
            filter_sustain: 0.6,
            filter_release: 0.25,
            amp_attack: 0.005,
            amp_decay: 0.1,
            amp_sustain: 0.9,
            amp_release: 0.2,
            distortion_enabled: true,
            distortion_type: DistortionTypeSer::Tanh,
            distortion_drive: 2.5,
            delay_enabled: true,
            delay_time_ms: 300.0,
            delay_feedback: 0.2,
            delay_mix: 0.15,
            ..Default::default()
        };
        thick_lead.save(&dir.join("thick_lead.json"))?;

        Ok(())
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            preset_dir: PathBuf::from("presets"),
            presets: Vec::new(),
            current_index: 0,
        })
    }
}
