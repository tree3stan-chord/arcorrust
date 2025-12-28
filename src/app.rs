//! Application state and lifecycle management

use crate::audio::{AudioEngine, Waveform};
use anyhow::Result;
use std::collections::HashSet;

/// Main application state
pub struct App {
    /// Audio engine for sound synthesis
    audio_engine: AudioEngine,
    /// Master volume (0.0 - 1.0)
    volume: f32,
    /// Current octave (0-8, default 4 = middle C)
    octave: i32,
    /// Currently held notes (MIDI note numbers)
    held_notes: HashSet<u8>,
    /// Number of active voices
    voice_count: usize,
    /// CPU usage estimate
    cpu_usage: f32,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let audio_engine = AudioEngine::new()?;

        Ok(Self {
            audio_engine,
            volume: 0.8,
            octave: 4,
            held_notes: HashSet::new(),
            voice_count: 0,
            cpu_usage: 0.0,
        })
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
    }

    /// Trigger a note on
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.held_notes.insert(note);
        self.audio_engine.note_on(note, velocity);
    }

    /// Trigger a note off
    pub fn note_off(&mut self, note: u8) {
        self.held_notes.remove(&note);
        self.audio_engine.note_off(note);
    }

    /// Stop all notes (panic button)
    pub fn panic(&mut self) {
        self.held_notes.clear();
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

    /// Get current waveform
    pub fn waveform(&self) -> Waveform {
        self.audio_engine.waveform()
    }

    /// Cycle to next waveform
    pub fn next_waveform(&mut self) {
        self.audio_engine.next_waveform();
    }

    /// Cycle to previous waveform
    pub fn prev_waveform(&mut self) {
        self.audio_engine.prev_waveform();
    }

    /// Get current detune
    pub fn detune(&self) -> f32 {
        self.audio_engine.detune()
    }

    /// Adjust detune
    pub fn adjust_detune(&mut self, delta: f32) {
        let current = self.audio_engine.detune();
        self.audio_engine.set_detune(current + delta);
    }
}
