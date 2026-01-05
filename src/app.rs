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

/// Modal panel for extended controls (always one active)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalPanel {
    /// Oscillator settings (sidebar)
    #[default]
    Oscillator,
    /// Filter settings (sidebar)
    Filter,
    /// Basic effects settings (sidebar - Distortion, Delay, Reverb)
    EffectsBasic,
    /// LFO configuration
    LFO,
    /// Modulation settings (Ring mod, FM)
    Modulation,
    /// Extended effects (Chorus, Phaser, Bitcrusher)
    EffectsExt,
    /// Performance settings (Portamento, Arpeggiator, Noise)
    Performance,
    /// Envelope ADSR settings (Amplitude and Filter)
    Envelope,
}

impl ModalPanel {
    /// Cycle to next modal
    pub fn next(&self) -> Self {
        match self {
            ModalPanel::Oscillator => ModalPanel::Filter,
            ModalPanel::Filter => ModalPanel::Envelope,
            ModalPanel::Envelope => ModalPanel::EffectsBasic,
            ModalPanel::EffectsBasic => ModalPanel::LFO,
            ModalPanel::LFO => ModalPanel::Modulation,
            ModalPanel::Modulation => ModalPanel::EffectsExt,
            ModalPanel::EffectsExt => ModalPanel::Performance,
            ModalPanel::Performance => ModalPanel::Oscillator,
        }
    }

    /// Cycle to previous modal
    pub fn prev(&self) -> Self {
        match self {
            ModalPanel::Oscillator => ModalPanel::Performance,
            ModalPanel::Filter => ModalPanel::Oscillator,
            ModalPanel::Envelope => ModalPanel::Filter,
            ModalPanel::EffectsBasic => ModalPanel::Envelope,
            ModalPanel::LFO => ModalPanel::EffectsBasic,
            ModalPanel::Modulation => ModalPanel::LFO,
            ModalPanel::EffectsExt => ModalPanel::Modulation,
            ModalPanel::Performance => ModalPanel::EffectsExt,
        }
    }

    /// Get modal name for display
    pub fn name(&self) -> &'static str {
        match self {
            ModalPanel::Oscillator => "Oscillator",
            ModalPanel::Filter => "Filter",
            ModalPanel::Envelope => "Envelope",
            ModalPanel::EffectsBasic => "Effects",
            ModalPanel::LFO => "LFO",
            ModalPanel::Modulation => "Modulation",
            ModalPanel::EffectsExt => "FX+",
            ModalPanel::Performance => "Performance",
        }
    }

    /// Check if this is a sidebar panel
    #[allow(dead_code)]
    pub fn is_sidebar(&self) -> bool {
        matches!(self, ModalPanel::Oscillator | ModalPanel::Filter | ModalPanel::EffectsBasic)
    }

    /// All modal panels for iteration
    #[allow(dead_code)]
    pub fn all() -> &'static [ModalPanel] {
        &[
            ModalPanel::Oscillator,
            ModalPanel::Filter,
            ModalPanel::Envelope,
            ModalPanel::EffectsBasic,
            ModalPanel::LFO,
            ModalPanel::Modulation,
            ModalPanel::EffectsExt,
            ModalPanel::Performance,
        ]
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
    /// Whether we're in edit mode (inside a panel editing parameters)
    edit_mode: bool,
    /// Current parameter index within the active panel
    param_index: usize,
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
            edit_mode: false,
            param_index: 0,
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
    pub fn osc_mix(&self) -> f32 {
        self.audio_engine.osc_mix()
    }

    /// Adjust oscillator mix
    pub fn adjust_osc_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_osc_mix(delta);
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

    // === Amplitude Envelope ADSR controls ===

    /// Get amplitude attack time in seconds
    pub fn amp_attack(&self) -> f32 {
        self.audio_engine.amp_attack()
    }

    /// Adjust amplitude attack time
    pub fn adjust_amp_attack(&mut self, delta: f32) {
        self.audio_engine.adjust_amp_attack(delta);
    }

    /// Get amplitude decay time in seconds
    pub fn amp_decay(&self) -> f32 {
        self.audio_engine.amp_decay()
    }

    /// Adjust amplitude decay time
    pub fn adjust_amp_decay(&mut self, delta: f32) {
        self.audio_engine.adjust_amp_decay(delta);
    }

    /// Get amplitude sustain level (0.0 - 1.0)
    pub fn amp_sustain(&self) -> f32 {
        self.audio_engine.amp_sustain()
    }

    /// Adjust amplitude sustain level
    pub fn adjust_amp_sustain(&mut self, delta: f32) {
        self.audio_engine.adjust_amp_sustain(delta);
    }

    /// Get amplitude release time in seconds
    pub fn amp_release(&self) -> f32 {
        self.audio_engine.amp_release()
    }

    /// Adjust amplitude release time
    pub fn adjust_amp_release(&mut self, delta: f32) {
        self.audio_engine.adjust_amp_release(delta);
    }

    // === Filter Envelope ADSR controls ===

    /// Get filter attack time in seconds
    pub fn filter_attack(&self) -> f32 {
        self.audio_engine.filter_attack()
    }

    /// Adjust filter attack time
    pub fn adjust_filter_attack(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_attack(delta);
    }

    /// Get filter decay time in seconds
    pub fn filter_decay(&self) -> f32 {
        self.audio_engine.filter_decay()
    }

    /// Adjust filter decay time
    pub fn adjust_filter_decay(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_decay(delta);
    }

    /// Get filter sustain level (0.0 - 1.0)
    pub fn filter_sustain(&self) -> f32 {
        self.audio_engine.filter_sustain()
    }

    /// Adjust filter sustain level
    pub fn adjust_filter_sustain(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_sustain(delta);
    }

    /// Get filter release time in seconds
    pub fn filter_release(&self) -> f32 {
        self.audio_engine.filter_release()
    }

    /// Adjust filter release time
    pub fn adjust_filter_release(&mut self, delta: f32) {
        self.audio_engine.adjust_filter_release(delta);
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
    pub fn delay_feedback(&self) -> f32 {
        self.audio_engine.delay_feedback()
    }

    /// Get delay mix
    pub fn delay_mix(&self) -> f32 {
        self.audio_engine.delay_mix()
    }

    /// Adjust delay mix
    pub fn adjust_delay_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_delay_mix(delta);
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
    #[allow(dead_code)]
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

    /// Get bitcrusher mix
    pub fn bitcrusher_mix(&self) -> f32 {
        self.audio_engine.bitcrusher_mix()
    }

    /// Adjust bitcrusher mix
    pub fn adjust_bitcrusher_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_bitcrusher_mix(delta);
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

    /// Adjust chorus voices
    pub fn adjust_chorus_voices(&mut self, delta: i8) {
        self.audio_engine.adjust_chorus_voices(delta);
    }

    /// Get chorus mix
    pub fn chorus_mix(&self) -> f32 {
        self.audio_engine.chorus_mix()
    }

    /// Adjust chorus mix
    pub fn adjust_chorus_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_chorus_mix(delta);
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

    /// Adjust ring mod mix
    pub fn adjust_ring_mod_mix(&mut self, delta: f32) {
        self.audio_engine.adjust_ring_mod_mix(delta);
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
    #[allow(dead_code)]
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

        // Amplitude envelope
        self.audio_engine.set_amp_attack(preset.amp_attack);
        self.audio_engine.set_amp_decay(preset.amp_decay);
        self.audio_engine.set_amp_sustain(preset.amp_sustain);
        self.audio_engine.set_amp_release(preset.amp_release);

        // Filter envelope
        self.audio_engine.set_filter_attack(preset.filter_attack);
        self.audio_engine.set_filter_decay(preset.filter_decay);
        self.audio_engine.set_filter_sustain(preset.filter_sustain);
        self.audio_engine.set_filter_release(preset.filter_release);

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
        self.param_index = 0;
        self.edit_mode = false; // Exit edit mode when changing panels
    }

    /// Cycle to previous modal
    pub fn prev_modal(&mut self) {
        self.modal_panel = self.modal_panel.prev();
        self.param_index = 0;
        self.edit_mode = false;
    }

    // === Edit mode controls ===

    /// Check if in edit mode (inside a panel)
    pub fn edit_mode(&self) -> bool {
        self.edit_mode
    }

    /// Get current parameter index
    pub fn param_index(&self) -> usize {
        self.param_index
    }

    /// Enter edit mode for current panel
    pub fn enter_edit_mode(&mut self) {
        self.edit_mode = true;
        self.param_index = 0;
    }

    /// Exit edit mode (back to panel navigation)
    pub fn exit_edit_mode(&mut self) {
        self.edit_mode = false;
    }

    /// Get parameter count for current modal panel
    pub fn param_count(&self) -> usize {
        match self.modal_panel {
            ModalPanel::Oscillator => 4,     // Wave1, Osc2, Wave2, Mix
            ModalPanel::Filter => 5,         // On/Off, Type, Cutoff, Resonance, EnvAmt
            ModalPanel::Envelope => 8,       // AmpA, AmpD, AmpS, AmpR, FltA, FltD, FltS, FltR
            ModalPanel::EffectsBasic => 6,   // Dist, Delay, DelTime, DelFB, DelMix, Reverb
            ModalPanel::LFO => 4,            // On/Off, Wave, Rate, Depth
            ModalPanel::Modulation => 5,     // Ring, RingFreq, RingMix, FM, FMRatio
            ModalPanel::EffectsExt => 8,     // Chorus, ChRate, ChDepth, ChVoices, ChMix, Phaser, Bit, BitMix
            ModalPanel::Performance => 6,    // Porta, Noise, NoiseType, Arp, Pattern, Octaves
        }
    }

    /// Move to next parameter (with wrap)
    pub fn next_param(&mut self) {
        let count = self.param_count();
        self.param_index = (self.param_index + 1) % count;
    }

    /// Move to previous parameter (with wrap)
    pub fn prev_param(&mut self) {
        let count = self.param_count();
        if self.param_index == 0 {
            self.param_index = count - 1;
        } else {
            self.param_index -= 1;
        }
    }

    /// Modify the current parameter (up = increase/toggle, down = decrease)
    /// For toggles and cycles, up cycles forward and down cycles backward (or toggles)
    pub fn modify_current_param(&mut self, increase: bool) {
        match self.modal_panel {
            ModalPanel::Oscillator => {
                // Params: 0=Wave1, 1=Osc2 on/off, 2=Wave2, 3=Mix
                match self.param_index {
                    0 => self.next_osc1_waveform(),
                    1 => self.toggle_osc2(),
                    2 => self.next_osc2_waveform(),
                    3 => self.adjust_osc_mix(if increase { 0.1 } else { -0.1 }),
                    _ => {}
                }
            }
            ModalPanel::Filter => {
                // Params: 0=On/Off, 1=Type, 2=Cutoff, 3=Resonance, 4=EnvAmt
                match self.param_index {
                    0 => self.toggle_filter(),
                    1 => self.next_filter_type(),
                    2 => self.adjust_filter_cutoff(if increase { 1.1 } else { 0.9 }),
                    3 => self.adjust_filter_resonance(if increase { 0.2 } else { -0.2 }),
                    4 => self.adjust_filter_env_amount(if increase { 0.1 } else { -0.1 }),
                    _ => {}
                }
            }
            ModalPanel::EffectsBasic => {
                // Params: 0=Dist, 1=Delay, 2=DelTime, 3=DelFB, 4=DelMix, 5=Reverb
                match self.param_index {
                    0 => self.toggle_distortion(),
                    1 => self.toggle_delay(),
                    2 => self.adjust_delay_time(if increase { 25.0 } else { -25.0 }),
                    3 => self.adjust_delay_feedback(if increase { 0.05 } else { -0.05 }),
                    4 => self.adjust_delay_mix(if increase { 0.1 } else { -0.1 }),
                    5 => self.toggle_reverb(),
                    _ => {}
                }
            }
            ModalPanel::LFO => {
                // Params: 0=On/Off, 1=Wave, 2=Rate, 3=Depth
                match self.param_index {
                    0 => self.toggle_lfo(),
                    1 => self.next_lfo_waveform(),
                    2 => self.adjust_lfo_rate(if increase { 0.5 } else { -0.5 }),
                    3 => self.adjust_lfo_depth(if increase { 0.1 } else { -0.1 }),
                    _ => {}
                }
            }
            ModalPanel::Modulation => {
                // Params: 0=Ring on/off, 1=Ring Freq, 2=Ring Mix, 3=FM on/off, 4=FM Ratio
                match self.param_index {
                    0 => self.toggle_ring_mod(),
                    1 => self.adjust_ring_mod_freq(if increase { 20.0 } else { -20.0 }),
                    2 => self.adjust_ring_mod_mix(if increase { 0.1 } else { -0.1 }),
                    3 => self.toggle_fm(),
                    4 => self.adjust_fm_ratio(if increase { 0.25 } else { -0.25 }),
                    _ => {}
                }
            }
            ModalPanel::EffectsExt => {
                // Params: 0=Chorus, 1=ChRate, 2=ChDepth, 3=ChVoices, 4=ChMix, 5=Phaser, 6=Bit, 7=BitMix
                match self.param_index {
                    0 => self.toggle_chorus(),
                    1 => self.adjust_chorus_rate(if increase { 0.2 } else { -0.2 }),
                    2 => self.adjust_chorus_depth(if increase { 0.1 } else { -0.1 }),
                    3 => self.adjust_chorus_voices(if increase { 1 } else { -1 }),
                    4 => self.adjust_chorus_mix(if increase { 0.1 } else { -0.1 }),
                    5 => self.toggle_phaser(),
                    6 => self.toggle_bitcrusher(),
                    7 => self.adjust_bitcrusher_mix(if increase { 0.1 } else { -0.1 }),
                    _ => {}
                }
            }
            ModalPanel::Performance => {
                // Params: 0=Porta, 1=Noise, 2=NoiseType, 3=Arp, 4=Pattern, 5=Octaves
                match self.param_index {
                    0 => self.next_portamento_mode(),
                    1 => self.toggle_noise(),
                    2 => self.next_noise_type(),
                    3 => self.toggle_arpeggiator(),
                    4 => self.next_arpeggiator_pattern(),
                    5 => self.adjust_arpeggiator_octaves(if increase { 1 } else { -1 }),
                    _ => {}
                }
            }
            ModalPanel::Envelope => {
                // Params: 0=AmpA, 1=AmpD, 2=AmpS, 3=AmpR, 4=FltA, 5=FltD, 6=FltS, 7=FltR
                let delta = if increase { 0.05 } else { -0.05 };
                match self.param_index {
                    0 => self.adjust_amp_attack(delta),
                    1 => self.adjust_amp_decay(delta),
                    2 => self.adjust_amp_sustain(delta),
                    3 => self.adjust_amp_release(delta),
                    4 => self.adjust_filter_attack(delta),
                    5 => self.adjust_filter_decay(delta),
                    6 => self.adjust_filter_sustain(delta),
                    7 => self.adjust_filter_release(delta),
                    _ => {}
                }
            }
        }
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn adjust_arpeggiator_gate(&mut self, delta: f32) {
        self.arpeggiator.adjust_gate(delta);
    }
}
