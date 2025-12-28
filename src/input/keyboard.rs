//! QWERTY keyboard to MIDI note mapping
//!
//! Layout mirrors a piano keyboard:
//! ```text
//!   2 3   5 6 7   9 0   =
//!  Q W E R T Y U I O P [ ]
//!   S D   G H J   L ;
//!  Z X C V B N M , . /
//! ```
//!
//! Lower row (Z-/) plays notes C to B in the current octave
//! Upper row (Q-]) plays notes C to B one octave higher
//! Black keys are on the number row and S D G H J L ; keys

/// Map a character to a MIDI note number
///
/// Returns None if the character doesn't map to a note
pub fn char_to_note(c: char, octave: i32) -> Option<u8> {
    let base_note = (octave * 12 + 12) as i32; // C0 = 12

    let offset = match c.to_ascii_lowercase() {
        // Lower row - white keys (C to B)
        'z' => Some(0),  // C
        'x' => Some(2),  // D
        'c' => Some(4),  // E
        'v' => Some(5),  // F
        'b' => Some(7),  // G
        'n' => Some(9),  // A
        'm' => Some(11), // B
        ',' => Some(12), // C+1
        '.' => Some(14), // D+1
        '/' => Some(16), // E+1

        // Lower row - black keys
        's' => Some(1),  // C#
        'd' => Some(3),  // D#
        'g' => Some(6),  // F#
        'h' => Some(8),  // G#
        'j' => Some(10), // A#
        'l' => Some(13), // C#+1
        ';' => Some(15), // D#+1

        // Upper row - white keys (C to B, one octave up)
        'q' => Some(12), // C
        'w' => Some(14), // D
        'e' => Some(16), // E
        'r' => Some(17), // F
        't' => Some(19), // G
        'y' => Some(21), // A
        'u' => Some(23), // B
        'i' => Some(24), // C+1
        'o' => Some(26), // D+1
        'p' => Some(28), // E+1
        '[' => Some(29), // F+1
        ']' => Some(31), // G+1

        // Upper row - black keys
        '2' => Some(13), // C#
        '3' => Some(15), // D#
        '5' => Some(18), // F#
        '6' => Some(20), // G#
        '7' => Some(22), // A#
        '9' => Some(25), // C#+1
        '0' => Some(27), // D#+1
        '=' => Some(30), // F#+1

        _ => None,
    };

    offset.map(|o| {
        let note = base_note + o;
        note.clamp(0, 127) as u8
    })
}

/// Get the note name for a MIDI note number
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (note / 12) as i32 - 1;
    let name = NAMES[(note % 12) as usize];
    format!("{}{}", name, octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middle_c() {
        // Middle C is C4 = MIDI note 60
        let note = char_to_note('z', 4).unwrap();
        assert_eq!(note, 60);
        assert_eq!(note_name(note), "C4");
    }

    #[test]
    fn test_octave_up() {
        // Q should be one octave above Z
        let lower = char_to_note('z', 4).unwrap();
        let upper = char_to_note('q', 4).unwrap();
        assert_eq!(upper - lower, 12);
    }

    #[test]
    fn test_black_keys() {
        // S is C# (one semitone above C)
        let c = char_to_note('z', 4).unwrap();
        let cs = char_to_note('s', 4).unwrap();
        assert_eq!(cs - c, 1);
    }
}
