//! Application state and lifecycle management

use crate::audio::{AudioEngine, FilterType, Waveform};
use crate::preset::{Preset, PresetManager};
use crate::recorder;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

/// How long to hold a note before auto-release (for terminals without key release support)
const NOTE_AUTO_RELEASE_MS: u128 = 200;

/// Main application state
pub struct App {
    /// Audio engine for sound synthesis
    audio_engine: AudioEngine,
    /// Preset manager
    preset_manager: PresetManager,
    /// Master volume (0.0 - 1.0)
    volume: f32,
    /// Current octave (0-8, default 4 = middle C)
    octave: i32,
    /// Currently held notes (MIDI note numbers)
    held_notes: HashSet<u8>,
    /// When each note was last pressed (for auto-release fallback)
    note_press_times: HashMap<u8, Instant>,
    /// Number of active voices
    voice_count: usize,
    /// CPU usage estimate
    cpu_usage: f32,
    /// Whether preset has been modified since loading
    preset_modified: bool,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let audio_engine = AudioEngine::new()?;
        let preset_manager = PresetManager::new()?;

        let mut app = Self {
            audio_engine,
            preset_manager,
            volume: 0.8,
            octave: 4,
            held_notes: HashSet::new(),
            note_press_times: HashMap::new(),
            voice_count: 0,
            cpu_usage: 0.0,
            preset_modified: false,
        };

        // Load first preset if available
        if let Ok(preset) = app.preset_manager.load_current() {
            app.apply_preset(&preset);
        }

        Ok(app)
    }

    /// Shutdown the application
    pub fn shutdown(&mut self) {
        self.audio_engine.stop();
    }

    /// Called every frame to update state
    pub fn tick(&mut self) {
        // Update voice count from audio engine
        self.voice_count = self.audio_engine.voice_count();
        // TODO: Calculate actual CPU usage
        self.cpu_usage = self.voice_count as f32 * 0.5;

        // Auto-release notes that have been held too long
        // (fallback for terminals that don't support key release events)
        let now = Instant::now();
        let notes_to_release: Vec<u8> = self
            .note_press_times
            .iter()
            .filter(|(_, &press_time)| now.duration_since(press_time).as_millis() > NOTE_AUTO_RELEASE_MS)
            .map(|(&note, _)| note)
            .collect();

        for note in notes_to_release {
            self.note_press_times.remove(&note);
            self.held_notes.remove(&note);
            self.audio_engine.note_off(note);
        }
    }

    /// Trigger a note on
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.held_notes.insert(note);
        self.note_press_times.insert(note, Instant::now());
        self.audio_engine.note_on(note, velocity);
    }

    /// Trigger a note off (called when key is released)
    pub fn note_off(&mut self, note: u8) {
        self.note_press_times.remove(&note);
        self.held_notes.remove(&note);
        self.audio_engine.note_off(note);
    }

    /// Stop all notes (panic button)
    pub fn panic(&mut self) {
        self.held_notes.clear();
        self.note_press_times.clear();
        self.audio_engine.panic();
    }

    /// Adjust master volume
    pub fn adjust_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        self.audio_engine.set_volume(self.volume);
    }

    /// Get current volume
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Get current octave
    pub fn octave(&self) -> i32 {
        self.octave
    }

    /// Decrease octave
    pub fn octave_down(&mut self) {
        if self.octave > 0 {
            self.octave -= 1;
        }
    }

    /// Increase octave
    pub fn octave_up(&mut self) {
        if self.octave < 8 {
            self.octave += 1;
        }
    }

    /// Get the number of active voices
    pub fn voice_count(&self) -> usize {
        self.voice_count
    }

    /// Get CPU usage estimate
    pub fn cpu_usage(&self) -> f32 {
        self.cpu_usage
    }

    /// Get currently held notes
    pub fn held_notes(&self) -> &HashSet<u8> {
        &self.held_notes
    }

    // === Oscillator 1 ===

    /// Get oscillator 1 waveform
    pub fn osc1_waveform(&self) -> Waveform {
        self.audio_engine.osc1_waveform()
    }

    /// Cycle oscillator 1 to next waveform
    pub fn next_osc1_waveform(&mut self) {
        self.audio_engine.next_osc1_waveform();
    }

    /// Cycle oscillator 1 to previous waveform
    pub fn prev_osc1_waveform(&mut self) {
        self.audio_engine.prev_osc1_waveform();
    }

    /// Get oscillator 1 detune
    pub fn osc1_detune(&self) -> f32 {
        self.audio_engine.osc1_detune()
    }

    // === Oscillator 2 ===

    /// Toggle oscillator 2
    pub fn toggle_osc2(&mut self) {
        self.audio_engine.toggle_osc2();
    }

    /// Get oscillator 2 enabled state
    pub fn osc2_enabled(&self) -> bool {
        self.audio_engine.osc2_enabled()
    }

    /// Get oscillator 2 waveform
    pub fn osc2_waveform(&self) -> Waveform {
        self.audio_engine.osc2_waveform()
    }

    /// Cycle oscillator 2 to next waveform
    pub fn next_osc2_waveform(&mut self) {
        self.audio_engine.next_osc2_waveform();
    }

    /// Get oscillator 2 detune
    pub fn osc2_detune(&self) -> f32 {
        self.audio_engine.osc2_detune()
    }

    /// Get oscillator 2 pitch offset
    pub fn osc2_pitch(&self) -> i32 {
        self.audio_engine.osc2_pitch()
    }

    /// Get oscillator mix
    pub fn osc_mix(&self) -> f32 {
        self.audio_engine.osc_mix()
    }

    // === Filter controls ===

    /// Get filter type
    pub fn filter_type(&self) -> FilterType {
        self.audio_engine.filter_type()
    }

    /// Cycle filter to next type
    pub fn next_filter_type(&mut self) {
        self.audio_engine.next_filter_type();
    }

    /// Cycle filter to previous type
    pub fn prev_filter_type(&mut self) {
        self.audio_engine.prev_filter_type();
    }

    /// Get filter cutoff frequency
    pub fn filter_cutoff(&self) -> f32 {
        self.audio_engine.filter_cutoff()
    }

    /// Adjust filter cutoff by a factor
    pub fn adjust_filter_cutoff(&mut self, factor: f32) {
        self.audio_engine.adjust_filter_cutoff(factor);
    }

    /// Get filter resonance
    pub fn filter_resonance(&self) -> f32 {
        self.audio_engine.filter_resonance()
    }

    /// Adjust filter resonance
    pub fn adjust_filter_resonance(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_resonance(delta);
    }

    /// Toggle filter on/off
    pub fn toggle_filter(&mut self) {
        self.audio_engine.toggle_filter();
    }

    /// Get filter enabled state
    pub fn filter_enabled(&self) -> bool {
        self.audio_engine.filter_enabled()
    }

    // === Filter envelope controls ===

    /// Get filter envelope amount
    pub fn filter_env_amount(&self) -> f32 {
        self.audio_engine.filter_env_amount()
    }

    /// Adjust filter envelope amount
    pub fn adjust_filter_env_amount(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_env_amount(delta);
    }

    // === Effects controls ===

    // -- Distortion --

    /// Toggle distortion
    pub fn toggle_distortion(&mut self) {
        self.audio_engine.toggle_distortion();
    }

    /// Check if distortion is enabled
    pub fn distortion_enabled(&self) -> bool {
        self.audio_engine.distortion_enabled()
    }

    /// Cycle distortion type
    pub fn next_distortion_type(&mut self) {
        self.audio_engine.next_distortion_type();
    }

    /// Get distortion type name
    pub fn distortion_type_name(&self) -> &'static str {
        self.audio_engine.distortion_type_name()
    }

    /// Adjust distortion drive
    pub fn adjust_distortion_drive(&mut self, delta: f32) {
        self.audio_engine.adjust_distortion_drive(delta);
    }

    /// Get distortion drive
    pub fn distortion_drive(&self) -> f32 {
        self.audio_engine.distortion_drive()
    }

    // -- Delay --

    /// Toggle delay
    pub fn toggle_delay(&mut self) {
        self.audio_engine.toggle_delay();
    }

    /// Check if delay is enabled
    pub fn delay_enabled(&self) -> bool {
        self.audio_engine.delay_enabled()
    }

    /// Adjust delay time
    pub fn adjust_delay_time(&mut self, delta_ms: f32) {
        self.audio_engine.adjust_delay_time(delta_ms);
    }

    /// Get delay time in ms
    pub fn delay_time_ms(&self) -> f32 {
        self.audio_engine.delay_time_ms()
    }

    /// Adjust delay feedback
    pub fn adjust_delay_feedback(&mut self, delta: f32) {
        self.audio_engine.adjust_delay_feedback(delta);
    }

    /// Get delay feedback
    pub fn delay_feedback(&self) -> f32 {
        self.audio_engine.delay_feedback()
    }

    // -- Reverb --

    /// Toggle reverb
    pub fn toggle_reverb(&mut self) {
        self.audio_engine.toggle_reverb();
    }

    /// Check if reverb is enabled
    pub fn reverb_enabled(&self) -> bool {
        self.audio_engine.reverb_enabled()
    }

    /// Adjust reverb decay
    pub fn adjust_reverb_decay(&mut self, delta: f32) {
        self.audio_engine.adjust_reverb_decay(delta);
    }

    /// Get reverb decay
    pub fn reverb_decay(&self) -> f32 {
        self.audio_engine.reverb_decay()
    }

    /// Adjust reverb mix
    pub fn adjust_reverb_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_reverb_mix(delta);
    }

    /// Get reverb mix
    pub fn reverb_mix(&self) -> f32 {
        self.audio_engine.reverb_mix()
    }

    // === Preset controls ===

    /// Get current preset name
    pub fn preset_name(&self) -> &str {
        self.preset_manager.current_name()
    }

    /// Get current preset index
    pub fn preset_index(&self) -> usize {
        self.preset_manager.current_index()
    }

    /// Get total number of presets
    pub fn preset_count(&self) -> usize {
        self.preset_manager.list().len()
    }

    /// Check if preset is modified
    pub fn preset_modified(&self) -> bool {
        self.preset_modified
    }

    /// Load next preset
    pub fn next_preset(&mut self) {
        self.preset_manager.next();
        if let Ok(preset) = self.preset_manager.load_current() {
            self.apply_preset(&preset);
            self.preset_modified = false;
        }
    }

    /// Load previous preset
    pub fn prev_preset(&mut self) {
        self.preset_manager.prev();
        if let Ok(preset) = self.preset_manager.load_current() {
            self.apply_preset(&preset);
            self.preset_modified = false;
        }
    }

    /// Apply a preset to the audio engine
    fn apply_preset(&mut self, preset: &Preset) {
        // Oscillators
        self.audio_engine.set_osc1_waveform(preset.osc1_waveform.into());
        self.audio_engine.set_osc1_detune(preset.osc1_detune);
        self.audio_engine.set_osc2_enabled(preset.osc2_enabled);
        self.audio_engine.set_osc2_waveform(preset.osc2_waveform.into());
        self.audio_engine.set_osc2_detune(preset.osc2_detune);
        self.audio_engine.set_osc2_pitch(preset.osc2_pitch);
        self.audio_engine.set_osc_mix(preset.osc_mix);

        // Filter
        self.audio_engine.set_filter_enabled(preset.filter_enabled);
        self.audio_engine.set_filter_type(preset.filter_type.into());
        self.audio_engine.set_filter_cutoff(preset.filter_cutoff);
        self.audio_engine.set_filter_resonance(preset.filter_resonance);
        self.audio_engine.set_filter_env_amount(preset.filter_env_amount);

        // Distortion
        self.audio_engine.set_distortion_enabled(preset.distortion_enabled);
        self.audio_engine.set_distortion_type(preset.distortion_type.into());
        self.audio_engine.set_distortion_drive(preset.distortion_drive);

        // Delay
        self.audio_engine.set_delay_enabled(preset.delay_enabled);
        self.audio_engine.set_delay_time(preset.delay_time_ms);
        self.audio_engine.set_delay_feedback(preset.delay_feedback);
        self.audio_engine.set_delay_mix(preset.delay_mix);

        // Reverb
        self.audio_engine.set_reverb_enabled(preset.reverb_enabled);
        self.audio_engine.set_reverb_decay(preset.reverb_decay);
        self.audio_engine.set_reverb_mix(preset.reverb_mix);

        // Volume
        self.volume = preset.volume;
        self.audio_engine.set_volume(preset.volume);

        log::info!("Loaded preset: {}", preset.name);
    }

    // === Recording controls ===

    /// Toggle recording on/off
    pub fn toggle_recording(&mut self) -> Option<PathBuf> {
        if self.audio_engine.is_recording() {
            // Stop recording and save
            let samples = self.audio_engine.stop_recording();
            if samples.is_empty() {
                return None;
            }

            // Save to file
            if let Ok(dir) = recorder::recordings_dir() {
                let filename = recorder::generate_filename();
                let path = dir.join(&filename);
                let sample_rate = self.audio_engine.sample_rate() as u32;

                if let Err(e) = recorder::save_wav(&samples, sample_rate, &path) {
                    log::error!("Failed to save recording: {}", e);
                    return None;
                }

                return Some(path);
            }
            None
        } else {
            // Start recording
            self.audio_engine.start_recording();
            None
        }
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.audio_engine.is_recording()
    }

    /// Get recording duration in seconds
    pub fn recording_duration(&self) -> f32 {
        self.audio_engine.recording_duration()
    }
}
