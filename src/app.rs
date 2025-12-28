//! Application state and lifecycle management

use crate::arpeggiator::{ArpPattern, Arpeggiator, NoteDivision};
use crate::audio::{AudioEngine, FilterType, Waveform};
use crate::preset::{Preset, PresetManager};
use crate::recorder;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

/// How long to hold a note before auto-release (for terminals without key release support)
const NOTE_AUTO_RELEASE_MS: u128 = 200;

/// Modal panel for extended controls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalPanel {
    /// No modal open
    #[default]
    None,
    /// LFO configuration
    LFO,
    /// Modulation settings (Ring mod, FM)
    Modulation,
    /// Effects settings (Chorus, Phaser, Bitcrusher)
    Effects,
    /// Performance settings (Portamento, Arpeggiator, Noise)
    Performance,
}

impl ModalPanel {
    /// Cycle to next modal
    pub fn next(&self) -> Self {
        match self {
            ModalPanel::None => ModalPanel::LFO,
            ModalPanel::LFO => ModalPanel::Modulation,
            ModalPanel::Modulation => ModalPanel::Effects,
            ModalPanel::Effects => ModalPanel::Performance,
            ModalPanel::Performance => ModalPanel::None,
        }
    }

    /// Cycle to previous modal
    pub fn prev(&self) -> Self {
        match self {
            ModalPanel::None => ModalPanel::Performance,
            ModalPanel::LFO => ModalPanel::None,
            ModalPanel::Modulation => ModalPanel::LFO,
            ModalPanel::Effects => ModalPanel::Modulation,
            ModalPanel::Performance => ModalPanel::Effects,
        }
    }

    /// Get modal name for display
    pub fn name(&self) -> &'static str {
        match self {
            ModalPanel::None => "None",
            ModalPanel::LFO => "LFO",
            ModalPanel::Modulation => "Modulation",
            ModalPanel::Effects => "Effects",
            ModalPanel::Performance => "Performance",
        }
    }

    /// Check if any modal is open
    pub fn is_open(&self) -> bool {
        !matches!(self, ModalPanel::None)
    }
}

/// UI layout mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Sidebar layout: controls on left, keyboard on right
    #[default]
    Sidebar,
    /// Wide layout: full-width panels stacked vertically
    Wide,
}

impl LayoutMode {
    pub fn toggle(&self) -> Self {
        match self {
            LayoutMode::Sidebar => LayoutMode::Wide,
            LayoutMode::Wide => LayoutMode::Sidebar,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            LayoutMode::Sidebar => "Sidebar",
            LayoutMode::Wide => "Wide",
        }
    }
}

/// Visualization mode for audio display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VizMode {
    /// No visualization
    Off,
    /// Waveform oscilloscope view
    Waveform,
    /// Frequency spectrum analyzer
    Spectrum,
    /// Both waveform and spectrum
    #[default]
    Combined,
}

impl VizMode {
    pub fn next(&self) -> Self {
        match self {
            VizMode::Off => VizMode::Waveform,
            VizMode::Waveform => VizMode::Spectrum,
            VizMode::Spectrum => VizMode::Combined,
            VizMode::Combined => VizMode::Off,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            VizMode::Off => "Off",
            VizMode::Waveform => "Wave",
            VizMode::Spectrum => "Spectrum",
            VizMode::Combined => "Both",
        }
    }
}

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
    /// UI layout mode
    layout_mode: LayoutMode,
    /// Visualization mode
    viz_mode: VizMode,
    /// Current modal panel
    modal_panel: ModalPanel,
    /// Arpeggiator
    arpeggiator: Arpeggiator,
    /// Last tick time for delta calculation
    last_tick_time: Instant,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let audio_engine = AudioEngine::new()?;
        let preset_manager = PresetManager::new()?;
        let sample_rate = audio_engine.sample_rate();

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
            layout_mode: LayoutMode::default(),
            viz_mode: VizMode::default(),
            modal_panel: ModalPanel::default(),
            arpeggiator: Arpeggiator::new(sample_rate),
            last_tick_time: Instant::now(),
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

        // Calculate delta time since last tick
        let now = Instant::now();
        let delta_ms = now.duration_since(self.last_tick_time).as_secs_f32() * 1000.0;
        self.last_tick_time = now;

        // Process arpeggiator
        if self.arpeggiator.enabled() {
            let (note_on, note_off) = self.arpeggiator.tick(delta_ms);

            // Handle note off first
            if let Some(off_note) = note_off {
                self.audio_engine.note_off(off_note);
            }

            // Handle note on
            if let Some(on_note) = note_on {
                self.audio_engine.note_on(on_note, 100);
            }
        }

        // Auto-release notes that have been held too long
        // (fallback for terminals that don't support key release events)
        // Only do this when arpeggiator is disabled
        if !self.arpeggiator.enabled() {
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
    }

    /// Trigger a note on
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.held_notes.insert(note);
        self.note_press_times.insert(note, Instant::now());

        if self.arpeggiator.enabled() {
            // When arpeggiator is enabled, add note to arpeggiator instead of triggering directly
            self.arpeggiator.note_on(note);
        } else {
            self.audio_engine.note_on(note, velocity);
        }
    }

    /// Trigger a note off (called when key is released)
    pub fn note_off(&mut self, note: u8) {
        self.note_press_times.remove(&note);
        self.held_notes.remove(&note);

        if self.arpeggiator.enabled() {
            // Remove note from arpeggiator
            self.arpeggiator.note_off(note);
        } else {
            self.audio_engine.note_off(note);
        }
    }

    /// Stop all notes (panic button)
    pub fn panic(&mut self) {
        self.held_notes.clear();
        self.note_press_times.clear();
        self.arpeggiator.clear();
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn osc2_detune(&self) -> f32 {
        self.audio_engine.osc2_detune()
    }

    /// Get oscillator 2 pitch offset
    #[allow(dead_code)]
    pub fn osc2_pitch(&self) -> i32 {
        self.audio_engine.osc2_pitch()
    }

    /// Get oscillator mix
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn adjust_delay_feedback(&mut self, delta: f32) {
        self.audio_engine.adjust_delay_feedback(delta);
    }

    /// Get delay feedback
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    // -- Bitcrusher --

    /// Toggle bitcrusher
    pub fn toggle_bitcrusher(&mut self) {
        self.audio_engine.toggle_bitcrusher();
    }

    /// Check if bitcrusher is enabled
    pub fn bitcrusher_enabled(&self) -> bool {
        self.audio_engine.bitcrusher_enabled()
    }

    /// Get bitcrusher bit depth
    pub fn bitcrusher_bits(&self) -> u8 {
        self.audio_engine.bitcrusher_bits()
    }

    /// Adjust bitcrusher bit depth
    pub fn adjust_bitcrusher_bits(&mut self, delta: i8) {
        self.audio_engine.adjust_bitcrusher_bits(delta);
    }

    /// Get bitcrusher sample rate divider
    pub fn bitcrusher_rate_div(&self) -> u8 {
        self.audio_engine.bitcrusher_rate_div()
    }

    /// Adjust bitcrusher sample rate divider
    pub fn adjust_bitcrusher_rate_div(&mut self, delta: i8) {
        self.audio_engine.adjust_bitcrusher_rate_div(delta);
    }

    // -- Chorus --

    /// Toggle chorus
    pub fn toggle_chorus(&mut self) {
        self.audio_engine.toggle_chorus();
    }

    /// Check if chorus is enabled
    pub fn chorus_enabled(&self) -> bool {
        self.audio_engine.chorus_enabled()
    }

    /// Get chorus rate
    pub fn chorus_rate(&self) -> f32 {
        self.audio_engine.chorus_rate()
    }

    /// Adjust chorus rate
    #[allow(dead_code)]
    pub fn adjust_chorus_rate(&mut self, delta: f32) {
        self.audio_engine.adjust_chorus_rate(delta);
    }

    /// Get chorus depth
    pub fn chorus_depth(&self) -> f32 {
        self.audio_engine.chorus_depth()
    }

    /// Adjust chorus depth
    #[allow(dead_code)]
    pub fn adjust_chorus_depth(&mut self, delta: f32) {
        self.audio_engine.adjust_chorus_depth(delta);
    }

    /// Get chorus voices
    pub fn chorus_voices(&self) -> u8 {
        self.audio_engine.chorus_voices()
    }

    // -- Phaser --

    /// Toggle phaser
    pub fn toggle_phaser(&mut self) {
        self.audio_engine.toggle_phaser();
    }

    /// Check if phaser is enabled
    pub fn phaser_enabled(&self) -> bool {
        self.audio_engine.phaser_enabled()
    }

    /// Get phaser rate
    pub fn phaser_rate(&self) -> f32 {
        self.audio_engine.phaser_rate()
    }

    /// Get phaser depth
    pub fn phaser_depth(&self) -> f32 {
        self.audio_engine.phaser_depth()
    }

    /// Get phaser stages
    pub fn phaser_stages(&self) -> u8 {
        self.audio_engine.phaser_stages()
    }

    // -- Ring Mod --

    /// Toggle ring mod
    pub fn toggle_ring_mod(&mut self) {
        self.audio_engine.toggle_ring_mod();
    }

    /// Check if ring mod is enabled
    pub fn ring_mod_enabled(&self) -> bool {
        self.audio_engine.ring_mod_enabled()
    }

    /// Get ring mod carrier frequency
    pub fn ring_mod_freq(&self) -> f32 {
        self.audio_engine.ring_mod_freq()
    }

    /// Adjust ring mod carrier frequency
    pub fn adjust_ring_mod_freq(&mut self, delta: f32) {
        self.audio_engine.adjust_ring_mod_freq(delta);
    }

    /// Get ring mod mix
    pub fn ring_mod_mix(&self) -> f32 {
        self.audio_engine.ring_mod_mix()
    }

    // -- FM Synthesis --

    /// Toggle FM synthesis
    pub fn toggle_fm(&mut self) {
        self.audio_engine.toggle_fm();
    }

    /// Check if FM synthesis is enabled
    pub fn fm_enabled(&self) -> bool {
        self.audio_engine.fm_enabled()
    }

    /// Get FM amount
    pub fn fm_amount(&self) -> f32 {
        self.audio_engine.fm_amount()
    }

    /// Adjust FM amount
    pub fn adjust_fm_amount(&mut self, delta: f32) {
        self.audio_engine.adjust_fm_amount(delta);
    }

    /// Get FM ratio
    pub fn fm_ratio(&self) -> f32 {
        self.audio_engine.fm_ratio()
    }

    /// Adjust FM ratio
    pub fn adjust_fm_ratio(&mut self, delta: f32) {
        self.audio_engine.adjust_fm_ratio(delta);
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

    // === Layout controls ===

    /// Get current layout mode
    pub fn layout_mode(&self) -> LayoutMode {
        self.layout_mode
    }

    /// Toggle between layout modes
    pub fn toggle_layout(&mut self) {
        self.layout_mode = self.layout_mode.toggle();
    }

    // === Visualization controls ===

    /// Get current visualization mode
    pub fn viz_mode(&self) -> VizMode {
        self.viz_mode
    }

    /// Cycle to next visualization mode
    pub fn next_viz_mode(&mut self) {
        self.viz_mode = self.viz_mode.next();
    }

    /// Get visualization samples from audio engine
    pub fn get_viz_samples(&self) -> Vec<f32> {
        self.audio_engine.get_viz_samples()
    }

    /// Get sample rate for spectrum calculations
    pub fn sample_rate(&self) -> f32 {
        self.audio_engine.sample_rate()
    }

    // === Modal controls ===

    /// Get current modal panel
    pub fn modal_panel(&self) -> ModalPanel {
        self.modal_panel
    }

    /// Cycle to next modal
    pub fn next_modal(&mut self) {
        self.modal_panel = self.modal_panel.next();
    }

    /// Cycle to previous modal
    pub fn prev_modal(&mut self) {
        self.modal_panel = self.modal_panel.prev();
    }

    /// Close current modal
    pub fn close_modal(&mut self) {
        self.modal_panel = ModalPanel::None;
    }

    /// Check if any modal is open
    pub fn modal_is_open(&self) -> bool {
        self.modal_panel.is_open()
    }

    // === LFO controls ===

    /// Toggle LFO on/off
    pub fn toggle_lfo(&mut self) {
        self.audio_engine.toggle_lfo();
    }

    /// Check if LFO is enabled
    pub fn lfo_enabled(&self) -> bool {
        self.audio_engine.lfo_enabled()
    }

    /// Get LFO rate
    pub fn lfo_rate(&self) -> f32 {
        self.audio_engine.lfo_rate()
    }

    /// Adjust LFO rate
    pub fn adjust_lfo_rate(&mut self, delta: f32) {
        self.audio_engine.adjust_lfo_rate(delta);
    }

    /// Get LFO depth
    pub fn lfo_depth(&self) -> f32 {
        self.audio_engine.lfo_depth()
    }

    /// Adjust LFO depth
    pub fn adjust_lfo_depth(&mut self, delta: f32) {
        self.audio_engine.adjust_lfo_depth(delta);
    }

    /// Get LFO waveform
    pub fn lfo_waveform(&self) -> crate::audio::LFOWaveform {
        self.audio_engine.lfo_waveform()
    }

    /// Cycle to next LFO waveform
    pub fn next_lfo_waveform(&mut self) {
        self.audio_engine.next_lfo_waveform();
    }

    /// Toggle LFO destination
    pub fn toggle_lfo_destination(&mut self, dest: crate::audio::LFODestination) {
        self.audio_engine.toggle_lfo_destination(dest);
    }

    /// Check if LFO destination is active
    pub fn lfo_has_destination(&self, dest: crate::audio::LFODestination) -> bool {
        self.audio_engine.lfo_has_destination(dest)
    }

    // === PWM controls ===

    /// Get pulse width
    #[allow(dead_code)]
    pub fn pulse_width(&self) -> f32 {
        self.audio_engine.pulse_width()
    }

    /// Adjust pulse width
    #[allow(dead_code)]
    pub fn adjust_pulse_width(&mut self, delta: f32) {
        self.audio_engine.adjust_pulse_width(delta);
    }

    // === Noise controls ===

    /// Toggle noise on/off
    pub fn toggle_noise(&mut self) {
        self.audio_engine.toggle_noise();
    }

    /// Check if noise is enabled
    pub fn noise_enabled(&self) -> bool {
        self.audio_engine.noise_enabled()
    }

    /// Get noise level
    pub fn noise_level(&self) -> f32 {
        self.audio_engine.noise_level()
    }

    /// Adjust noise level
    pub fn adjust_noise_level(&mut self, delta: f32) {
        self.audio_engine.adjust_noise_level(delta);
    }

    /// Get noise type
    pub fn noise_type(&self) -> crate::audio::NoiseType {
        self.audio_engine.noise_type()
    }

    /// Cycle to next noise type
    pub fn next_noise_type(&mut self) {
        self.audio_engine.next_noise_type();
    }

    // === Portamento controls ===

    /// Get portamento time in milliseconds
    pub fn portamento_time(&self) -> f32 {
        self.audio_engine.portamento_time()
    }

    /// Adjust portamento time
    pub fn adjust_portamento_time(&mut self, delta: f32) {
        self.audio_engine.adjust_portamento_time(delta);
    }

    /// Get portamento mode
    pub fn portamento_mode(&self) -> crate::audio::PortamentoMode {
        self.audio_engine.portamento_mode()
    }

    /// Cycle to next portamento mode
    pub fn next_portamento_mode(&mut self) {
        self.audio_engine.next_portamento_mode();
    }

    // === Arpeggiator controls ===

    /// Toggle arpeggiator on/off
    pub fn toggle_arpeggiator(&mut self) {
        self.arpeggiator.toggle();
        // If disabling, make sure all held notes are passed to the engine
        if !self.arpeggiator.enabled() {
            self.audio_engine.panic();
        }
    }

    /// Check if arpeggiator is enabled
    pub fn arpeggiator_enabled(&self) -> bool {
        self.arpeggiator.enabled()
    }

    /// Get arpeggiator BPM
    pub fn arpeggiator_bpm(&self) -> f32 {
        self.arpeggiator.bpm()
    }

    /// Adjust arpeggiator BPM
    pub fn adjust_arpeggiator_bpm(&mut self, delta: f32) {
        self.arpeggiator.adjust_bpm(delta);
    }

    /// Get arpeggiator pattern
    pub fn arpeggiator_pattern(&self) -> ArpPattern {
        self.arpeggiator.pattern()
    }

    /// Cycle to next arpeggiator pattern
    pub fn next_arpeggiator_pattern(&mut self) {
        self.arpeggiator.next_pattern();
    }

    /// Get arpeggiator division
    pub fn arpeggiator_division(&self) -> NoteDivision {
        self.arpeggiator.division()
    }

    /// Cycle to next arpeggiator division
    pub fn next_arpeggiator_division(&mut self) {
        self.arpeggiator.next_division();
    }

    /// Get arpeggiator octaves
    pub fn arpeggiator_octaves(&self) -> u8 {
        self.arpeggiator.octaves()
    }

    /// Adjust arpeggiator octaves
    pub fn adjust_arpeggiator_octaves(&mut self, delta: i8) {
        self.arpeggiator.adjust_octaves(delta);
    }

    /// Get arpeggiator gate
    pub fn arpeggiator_gate(&self) -> f32 {
        self.arpeggiator.gate()
    }

    /// Adjust arpeggiator gate
    pub fn adjust_arpeggiator_gate(&mut self, delta: f32) {
        self.arpeggiator.adjust_gate(delta);
    }
}
