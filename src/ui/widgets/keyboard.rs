//! Piano keyboard widget with visual keys

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use std::collections::HashSet;

/// Width of a white key in characters
const WHITE_KEY_WIDTH: u16 = 6;
/// Height of a white key
const WHITE_KEY_HEIGHT: u16 = 6;

/// A visual piano keyboard widget
pub struct PianoKeyboard<'a> {
    /// Currently held notes (MIDI numbers)
    held_notes: &'a HashSet<u8>,
    /// Current octave for display
    octave: i32,
    /// Number of octaves to display
    num_octaves: u16,
}

impl<'a> PianoKeyboard<'a> {
    pub fn new(held_notes: &'a HashSet<u8>, octave: i32) -> Self {
        Self {
            held_notes,
            octave,
            num_octaves: 2,
        }
    }

    pub fn octaves(mut self, n: u16) -> Self {
        self.num_octaves = n;
        self
    }

    /// Get MIDI note for a given octave and note index (0-11)
    fn midi_note(octave: i32, note_in_octave: u8) -> u8 {
        ((octave + 1) * 12 + note_in_octave as i32).clamp(0, 127) as u8
    }

    /// Get the QWERTY key label for a note
    fn get_key_label(note_in_octave: u8, is_upper_row: bool) -> &'static str {
        if is_upper_row {
            // Upper octave (Q row)
            match note_in_octave {
                0 => "Q",   // C
                1 => "2",   // C#
                2 => "W",   // D
                3 => "3",   // D#
                4 => "E",   // E
                5 => "R",   // F
                6 => "5",   // F#
                7 => "T",   // G
                8 => "6",   // G#
                9 => "Y",   // A
                10 => "7",  // A#
                11 => "U",  // B
                _ => "",
            }
        } else {
            // Lower octave (Z row)
            match note_in_octave {
                0 => "Z",   // C
                1 => "S",   // C#
                2 => "X",   // D
                3 => "D",   // D#
                4 => "C",   // E
                5 => "V",   // F
                6 => "G",   // F#
                7 => "B",   // G
                8 => "H",   // G#
                9 => "N",   // A
                10 => "J",  // A#
                11 => "M",  // B
                _ => "",
            }
        }
    }

    /// Get note name
    fn note_name(note_in_octave: u8) -> &'static str {
        match note_in_octave {
            0 => "C",
            1 => "C#",
            2 => "D",
            3 => "D#",
            4 => "E",
            5 => "F",
            6 => "F#",
            7 => "G",
            8 => "G#",
            9 => "A",
            10 => "A#",
            11 => "B",
            _ => "",
        }
    }

    /// Draw a single white key
    fn draw_white_key(
        buf: &mut Buffer,
        x: u16,
        y: u16,
        height: u16,
        label: &str,
        note_name: &str,
        is_pressed: bool,
        octave_num: i32,
    ) {
        let style = if is_pressed {
            Style::default().bg(Color::Green).fg(Color::Black)
        } else {
            Style::default().bg(Color::White).fg(Color::Black)
        };

        let border_style = Style::default().fg(Color::DarkGray);

        // Draw key body
        for row in 0..height {
            let cy = y + row;
            if cy >= buf.area.bottom() {
                break;
            }

            for col in 0..WHITE_KEY_WIDTH {
                let cx = x + col;
                if cx >= buf.area.right() {
                    break;
                }

                let cell = &mut buf[(cx, cy)];

                if row == 0 {
                    // Top border
                    if col == 0 {
                        cell.set_char('┌').set_style(border_style);
                    } else if col == WHITE_KEY_WIDTH - 1 {
                        cell.set_char('┐').set_style(border_style);
                    } else {
                        cell.set_char('─').set_style(border_style);
                    }
                } else if row == height - 1 {
                    // Bottom border
                    if col == 0 {
                        cell.set_char('└').set_style(border_style);
                    } else if col == WHITE_KEY_WIDTH - 1 {
                        cell.set_char('┘').set_style(border_style);
                    } else {
                        cell.set_char('─').set_style(border_style);
                    }
                } else if col == 0 || col == WHITE_KEY_WIDTH - 1 {
                    // Side borders
                    cell.set_char('│').set_style(border_style);
                } else {
                    // Interior
                    cell.set_char(' ').set_style(style);
                }
            }
        }

        // Draw key label in the middle
        let label_y = y + height - 3;
        let label_x = x + (WHITE_KEY_WIDTH / 2);
        if label_y < buf.area.bottom() && label_x < buf.area.right() {
            let cell = &mut buf[(label_x, label_y)];
            if !label.is_empty() {
                cell.set_char(label.chars().next().unwrap())
                    .set_style(style.add_modifier(Modifier::BOLD));
            }
        }

        // Draw note name at bottom
        let note_y = y + height - 2;
        if note_y < buf.area.bottom() && note_name.len() == 1 {
            let note_x = x + (WHITE_KEY_WIDTH / 2);
            if note_x < buf.area.right() {
                buf[(note_x, note_y)]
                    .set_char(note_name.chars().next().unwrap())
                    .set_style(Style::default().fg(Color::DarkGray));
            }
        }

        // Show octave number on C keys
        if note_name == "C" {
            let oct_y = y + height - 2;
            let oct_x = x + WHITE_KEY_WIDTH - 2;
            if oct_y < buf.area.bottom() && oct_x < buf.area.right() {
                let oct_char = char::from_digit(octave_num as u32 % 10, 10).unwrap_or('?');
                buf[(oct_x, oct_y)]
                    .set_char(oct_char)
                    .set_style(Style::default().fg(Color::Blue));
            }
        }
    }

    /// Draw a single black key
    fn draw_black_key(
        buf: &mut Buffer,
        x: u16,
        y: u16,
        height: u16,
        label: &str,
        is_pressed: bool,
    ) {
        let style = if is_pressed {
            Style::default().bg(Color::Green).fg(Color::White)
        } else {
            Style::default().bg(Color::Black).fg(Color::White)
        };

        let width: u16 = 4;

        // Draw key body
        for row in 0..height {
            let cy = y + row;
            if cy >= buf.area.bottom() {
                break;
            }

            for col in 0..width {
                let cx = x + col;
                if cx >= buf.area.right() || cx < buf.area.left() {
                    continue;
                }

                let cell = &mut buf[(cx, cy)];

                if row == 0 {
                    // Top border
                    if col == 0 {
                        cell.set_char('┌').set_style(style);
                    } else if col == width - 1 {
                        cell.set_char('┐').set_style(style);
                    } else {
                        cell.set_char('─').set_style(style);
                    }
                } else if row == height - 1 {
                    // Bottom border
                    if col == 0 {
                        cell.set_char('└').set_style(style);
                    } else if col == width - 1 {
                        cell.set_char('┘').set_style(style);
                    } else {
                        cell.set_char('─').set_style(style);
                    }
                } else if col == 0 || col == width - 1 {
                    // Side borders
                    cell.set_char('│').set_style(style);
                } else {
                    // Interior
                    cell.set_char('█').set_style(style);
                }
            }
        }

        // Draw label
        let label_y = y + height - 2;
        let label_x = x + 1;
        if label_y < buf.area.bottom() && label_x < buf.area.right() && !label.is_empty() {
            buf[(label_x, label_y)]
                .set_char(label.chars().next().unwrap())
                .set_style(style.add_modifier(Modifier::BOLD));
        }
    }
}

impl<'a> Widget for PianoKeyboard<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 4 {
            return;
        }

        let key_height = (area.height).min(WHITE_KEY_HEIGHT);
        let black_height = (key_height * 2 / 3).max(2);

        // White key positions for one octave (C, D, E, F, G, A, B)
        let white_notes: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
        // Black key positions relative to white keys
        let black_offsets: [(u8, i16); 5] = [
            (1, 0),   // C# after C
            (3, 1),   // D# after D
            (6, 3),   // F# after F
            (8, 4),   // G# after G
            (10, 5),  // A# after A
        ];

        let mut x_offset: u16 = area.x + 1;

        // Draw octaves
        for oct_idx in 0..self.num_octaves {
            let current_octave = self.octave + oct_idx as i32;
            let is_upper = oct_idx == 1;

            // Draw white keys first
            for (i, &note_in_oct) in white_notes.iter().enumerate() {
                let midi = Self::midi_note(current_octave, note_in_oct);
                let is_pressed = self.held_notes.contains(&midi);
                let label = Self::get_key_label(note_in_oct, is_upper);
                let note_name = Self::note_name(note_in_oct);

                let key_x = x_offset + (i as u16 * WHITE_KEY_WIDTH);
                if key_x + WHITE_KEY_WIDTH > area.right() {
                    break;
                }

                Self::draw_white_key(
                    buf,
                    key_x,
                    area.y,
                    key_height,
                    label,
                    note_name,
                    is_pressed,
                    current_octave,
                );
            }

            // Draw black keys on top
            for &(note_in_oct, white_idx) in &black_offsets {
                let midi = Self::midi_note(current_octave, note_in_oct);
                let is_pressed = self.held_notes.contains(&midi);
                let label = Self::get_key_label(note_in_oct, is_upper);

                // Position black key between white keys
                let key_x = x_offset + (white_idx as u16 * WHITE_KEY_WIDTH) + WHITE_KEY_WIDTH - 2;
                if key_x + 4 > area.right() {
                    break;
                }

                Self::draw_black_key(buf, key_x, area.y, black_height, label, is_pressed);
            }

            x_offset += 7 * WHITE_KEY_WIDTH;
        }
    }
}
