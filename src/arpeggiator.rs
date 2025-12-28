//! Arpeggiator implementation for the synthesizer

use std::time::{SystemTime, UNIX_EPOCH};

/// Simple XorShift pseudo-random number generator
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new() -> Self {
        // Seed from current time
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(12345);
        Self { state: if seed == 0 { 1 } else { seed } }
    }

    fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn range(&mut self, max: usize) -> usize {
        (self.next() as usize) % max
    }
}

/// Arpeggiator pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArpPattern {
    /// Play notes from lowest to highest
    #[default]
    Up,
    /// Play notes from highest to lowest
    Down,
    /// Play notes up then down
    UpDown,
    /// Play notes in random order
    Random,
    /// Play notes in the order they were pressed
    AsPlayed,
}

impl ArpPattern {
    /// Get the display name for this pattern
    pub fn name(&self) -> &'static str {
        match self {
            ArpPattern::Up => "Up",
            ArpPattern::Down => "Down",
            ArpPattern::UpDown => "Up/Dn",
            ArpPattern::Random => "Random",
            ArpPattern::AsPlayed => "Order",
        }
    }

    /// Cycle to next pattern
    pub fn next(&self) -> Self {
        match self {
            ArpPattern::Up => ArpPattern::Down,
            ArpPattern::Down => ArpPattern::UpDown,
            ArpPattern::UpDown => ArpPattern::Random,
            ArpPattern::Random => ArpPattern::AsPlayed,
            ArpPattern::AsPlayed => ArpPattern::Up,
        }
    }
}

/// Note division options for arpeggiator timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteDivision {
    /// Quarter notes (1/4)
    Quarter,
    /// Eighth notes (1/8)
    #[default]
    Eighth,
    /// Sixteenth notes (1/16)
    Sixteenth,
    /// Thirty-second notes (1/32)
    ThirtySecond,
    /// Eighth note triplets (1/12)
    EighthTriplet,
    /// Sixteenth note triplets (1/24)
    SixteenthTriplet,
}

impl NoteDivision {
    /// Get the display name for this division
    pub fn name(&self) -> &'static str {
        match self {
            NoteDivision::Quarter => "1/4",
            NoteDivision::Eighth => "1/8",
            NoteDivision::Sixteenth => "1/16",
            NoteDivision::ThirtySecond => "1/32",
            NoteDivision::EighthTriplet => "1/8T",
            NoteDivision::SixteenthTriplet => "1/16T",
        }
    }

    /// Get notes per beat for this division
    pub fn notes_per_beat(&self) -> f32 {
        match self {
            NoteDivision::Quarter => 1.0,
            NoteDivision::Eighth => 2.0,
            NoteDivision::Sixteenth => 4.0,
            NoteDivision::ThirtySecond => 8.0,
            NoteDivision::EighthTriplet => 3.0,
            NoteDivision::SixteenthTriplet => 6.0,
        }
    }

    /// Cycle to next division
    pub fn next(&self) -> Self {
        match self {
            NoteDivision::Quarter => NoteDivision::Eighth,
            NoteDivision::Eighth => NoteDivision::Sixteenth,
            NoteDivision::Sixteenth => NoteDivision::ThirtySecond,
            NoteDivision::ThirtySecond => NoteDivision::EighthTriplet,
            NoteDivision::EighthTriplet => NoteDivision::SixteenthTriplet,
            NoteDivision::SixteenthTriplet => NoteDivision::Quarter,
        }
    }
}

/// Direction for UpDown pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum UpDownDirection {
    Up,
    Down,
}

/// Arpeggiator state
#[derive(Debug)]
#[allow(dead_code)]
pub struct Arpeggiator {
    /// Whether arpeggiator is enabled
    enabled: bool,
    /// Tempo in beats per minute
    bpm: f32,
    /// Note division
    division: NoteDivision,
    /// Arpeggio pattern
    pattern: ArpPattern,
    /// Number of octaves to span (1-4)
    octaves: u8,
    /// Gate length (0.1 - 1.0, proportion of note duration)
    gate: f32,

    /// Notes currently held by the user (in order pressed)
    held_notes: Vec<u8>,
    /// Current step in the sequence
    current_step: usize,
    /// Direction for UpDown pattern
    up_down_direction: UpDownDirection,
    /// Time accumulator in milliseconds
    time_accum_ms: f32,
    /// Current note being played (if any)
    current_playing_note: Option<u8>,
    /// Time the current note has been playing
    note_time_ms: f32,
    /// Sample rate for timing calculations
    sample_rate: f32,
}

impl Default for Arpeggiator {
    fn default() -> Self {
        Self {
            enabled: false,
            bpm: 120.0,
            division: NoteDivision::default(),
            pattern: ArpPattern::default(),
            octaves: 1,
            gate: 0.75,
            held_notes: Vec::new(),
            current_step: 0,
            up_down_direction: UpDownDirection::Up,
            time_accum_ms: 0.0,
            current_playing_note: None,
            note_time_ms: 0.0,
            sample_rate: 44100.0,
        }
    }
}

#[allow(dead_code)]
impl Arpeggiator {
    /// Create a new arpeggiator with the given sample rate
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Toggle arpeggiator on/off
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.reset();
        }
    }

    /// Check if arpeggiator is enabled
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Set enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    /// Get BPM
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set BPM
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(30.0, 300.0);
    }

    /// Adjust BPM
    pub fn adjust_bpm(&mut self, delta: f32) {
        self.set_bpm(self.bpm + delta);
    }

    /// Get division
    pub fn division(&self) -> NoteDivision {
        self.division
    }

    /// Set division
    pub fn set_division(&mut self, division: NoteDivision) {
        self.division = division;
    }

    /// Cycle to next division
    pub fn next_division(&mut self) {
        self.division = self.division.next();
    }

    /// Get pattern
    pub fn pattern(&self) -> ArpPattern {
        self.pattern
    }

    /// Set pattern
    pub fn set_pattern(&mut self, pattern: ArpPattern) {
        self.pattern = pattern;
    }

    /// Cycle to next pattern
    pub fn next_pattern(&mut self) {
        self.pattern = self.pattern.next();
    }

    /// Get octaves
    pub fn octaves(&self) -> u8 {
        self.octaves
    }

    /// Set octaves
    pub fn set_octaves(&mut self, octaves: u8) {
        self.octaves = octaves.clamp(1, 4);
    }

    /// Adjust octaves
    pub fn adjust_octaves(&mut self, delta: i8) {
        let new_octaves = (self.octaves as i8 + delta).clamp(1, 4) as u8;
        self.set_octaves(new_octaves);
    }

    /// Get gate
    pub fn gate(&self) -> f32 {
        self.gate
    }

    /// Set gate
    pub fn set_gate(&mut self, gate: f32) {
        self.gate = gate.clamp(0.1, 1.0);
    }

    /// Adjust gate
    pub fn adjust_gate(&mut self, delta: f32) {
        self.set_gate(self.gate + delta);
    }

    /// Add a held note
    pub fn note_on(&mut self, note: u8) {
        if !self.held_notes.contains(&note) {
            self.held_notes.push(note);
        }
    }

    /// Remove a held note
    pub fn note_off(&mut self, note: u8) {
        self.held_notes.retain(|&n| n != note);
        // If the note being released is the current playing note, stop it
        if self.current_playing_note == Some(note) {
            self.current_playing_note = None;
        }
    }

    /// Clear all held notes
    pub fn clear(&mut self) {
        self.held_notes.clear();
        self.reset();
    }

    /// Reset arpeggiator state
    fn reset(&mut self) {
        self.current_step = 0;
        self.up_down_direction = UpDownDirection::Up;
        self.time_accum_ms = 0.0;
        self.current_playing_note = None;
        self.note_time_ms = 0.0;
    }

    /// Get the duration of one step in milliseconds
    fn step_duration_ms(&self) -> f32 {
        // BPM / 60 = beats per second
        // notes_per_beat = how many notes per beat
        // step_duration = (60000 / bpm) / notes_per_beat
        (60000.0 / self.bpm) / self.division.notes_per_beat()
    }

    /// Build the sequence of notes to play based on pattern
    fn build_sequence(&self) -> Vec<u8> {
        if self.held_notes.is_empty() {
            return Vec::new();
        }

        // First, get base notes sorted by pitch
        let mut base_notes = self.held_notes.clone();

        // For patterns that need sorting
        match self.pattern {
            ArpPattern::Up | ArpPattern::Down | ArpPattern::UpDown => {
                base_notes.sort();
            }
            ArpPattern::Random | ArpPattern::AsPlayed => {
                // Keep original order
            }
        }

        // Expand across octaves
        let mut sequence = Vec::new();
        for octave_offset in 0..self.octaves {
            for &note in &base_notes {
                let transposed = note.saturating_add(octave_offset * 12);
                if transposed <= 127 {
                    sequence.push(transposed);
                }
            }
        }

        // Apply pattern direction
        match self.pattern {
            ArpPattern::Down => {
                sequence.reverse();
            }
            ArpPattern::UpDown => {
                // For UpDown, we play up then down (excluding repeated end notes)
                if sequence.len() > 1 {
                    let mut down = sequence.clone();
                    down.reverse();
                    // Remove first element to avoid repeating the top note
                    if down.len() > 1 {
                        down.remove(0);
                    }
                    // Remove last element to avoid repeating the bottom note
                    if !down.is_empty() && sequence.len() > 1 {
                        down.pop();
                    }
                    sequence.extend(down);
                }
            }
            ArpPattern::Random => {
                // Shuffle the sequence using Fisher-Yates
                let mut rng = SimpleRng::new();
                for i in (1..sequence.len()).rev() {
                    let j = rng.range(i + 1);
                    sequence.swap(i, j);
                }
            }
            _ => {}
        }

        sequence
    }

    /// Get the next note in the sequence
    fn get_next_note(&mut self) -> Option<u8> {
        let sequence = self.build_sequence();
        if sequence.is_empty() {
            return None;
        }

        // For Random pattern, rebuild sequence each time
        if self.pattern == ArpPattern::Random {
            let sequence = self.build_sequence();
            if !sequence.is_empty() {
                self.current_step = 0;
                return Some(sequence[0]);
            }
            return None;
        }

        // Wrap around if needed
        if self.current_step >= sequence.len() {
            self.current_step = 0;
        }

        let note = sequence.get(self.current_step).copied();
        self.current_step += 1;

        note
    }

    /// Tick the arpeggiator - call this from the app's tick function
    /// Returns (note_on, note_off) - the note to trigger on and/or off
    pub fn tick(&mut self, delta_ms: f32) -> (Option<u8>, Option<u8>) {
        if !self.enabled || self.held_notes.is_empty() {
            // If disabled or no notes held, just return any currently playing note as note_off
            let note_off = self.current_playing_note.take();
            return (None, note_off);
        }

        let step_duration = self.step_duration_ms();
        let gate_duration = step_duration * self.gate;

        self.time_accum_ms += delta_ms;
        self.note_time_ms += delta_ms;

        let mut note_on = None;
        let mut note_off = None;

        // Check if we need to release the current note (gate time elapsed)
        if let Some(playing) = self.current_playing_note {
            if self.note_time_ms >= gate_duration {
                note_off = Some(playing);
                self.current_playing_note = None;
            }
        }

        // Check if we need to trigger a new note (step time elapsed)
        if self.time_accum_ms >= step_duration {
            self.time_accum_ms -= step_duration;
            self.note_time_ms = 0.0;

            // Release previous note if still playing
            if let Some(playing) = self.current_playing_note.take() {
                note_off = Some(playing);
            }

            // Trigger next note
            if let Some(next_note) = self.get_next_note() {
                note_on = Some(next_note);
                self.current_playing_note = Some(next_note);
            }
        }

        (note_on, note_off)
    }

    /// Check if the arpeggiator has any held notes
    pub fn has_notes(&self) -> bool {
        !self.held_notes.is_empty()
    }

    /// Get the currently playing note (if any)
    pub fn current_note(&self) -> Option<u8> {
        self.current_playing_note
    }
}
