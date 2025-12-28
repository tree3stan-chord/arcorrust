//! arcorrust - Terminal-based virtual instrument playground
//!
//! A Rust TUI synthesizer with real-time audio synthesis.

mod app;
mod audio;
mod input;
mod preset;
mod recorder;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

fn main() -> Result<()> {
    // Disable logging to stdout - it interferes with TUI
    // TODO: Log to file instead if needed for debugging

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Enable keyboard enhancement for key release events (supported on kitty, WezTerm, iTerm2, etc.)
    let keyboard_enhancement_supported = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    )
    .is_ok();

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new()?;
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    if keyboard_enhancement_supported {
        execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Shutdown audio
    app.shutdown();

    if let Err(err) = result {
        log::error!("Application error: {}", err);
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Poll for events with timeout for responsive UI
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release)
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        // Esc = quit (don't use letter keys - they're for playing!)
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        // Space = panic (stop all notes)
                        KeyCode::Char(' ') => {
                            app.panic();
                        }
                        // F1 = toggle recording
                        KeyCode::F(1) => {
                            if let Some(path) = app.toggle_recording() {
                                log::info!("Recording saved to: {:?}", path);
                            }
                        }
                        // Arrow keys for controls (don't conflict with music keys)
                        KeyCode::Up => {
                            app.adjust_volume(0.05);
                        }
                        KeyCode::Down => {
                            app.adjust_volume(-0.05);
                        }
                        KeyCode::Left => {
                            app.octave_down();
                        }
                        KeyCode::Right => {
                            app.octave_up();
                        }
                        // PageUp/PageDown for preset navigation
                        KeyCode::PageUp => {
                            app.prev_preset();
                        }
                        KeyCode::PageDown => {
                            app.next_preset();
                        }
                        // Tab cycles osc1 waveform
                        KeyCode::Tab => {
                            app.next_osc1_waveform();
                        }
                        // Backtab (Shift+Tab) cycles osc1 waveform backwards
                        KeyCode::BackTab => {
                            app.prev_osc1_waveform();
                        }
                        // F2 toggles osc2
                        KeyCode::F(2) => {
                            app.toggle_osc2();
                        }
                        // F3 cycles osc2 waveform
                        KeyCode::F(3) => {
                            app.next_osc2_waveform();
                        }
                        // F4 toggles filter
                        KeyCode::F(4) => {
                            app.toggle_filter();
                        }
                        // F5 cycles filter type
                        KeyCode::F(5) => {
                            app.next_filter_type();
                        }
                        // F6 decreases cutoff
                        KeyCode::F(6) => {
                            app.adjust_filter_cutoff(0.8); // Decrease by 20%
                        }
                        // F7 increases cutoff
                        KeyCode::F(7) => {
                            app.adjust_filter_cutoff(1.25); // Increase by 25%
                        }
                        // F8 decreases resonance
                        KeyCode::F(8) => {
                            app.adjust_filter_resonance(-0.5);
                        }
                        // F9 increases resonance
                        KeyCode::F(9) => {
                            app.adjust_filter_resonance(0.5);
                        }
                        // F10 decreases filter envelope amount
                        KeyCode::F(10) => {
                            app.adjust_filter_env_amount(-0.1);
                        }
                        // F11 increases filter envelope amount
                        KeyCode::F(11) => {
                            app.adjust_filter_env_amount(0.1);
                        }
                        // F12 toggle distortion
                        KeyCode::F(12) => {
                            app.toggle_distortion();
                        }
                        // All other characters go to the musical keyboard
                        KeyCode::Char(c) => {
                            if let Some(note) = input::keyboard::char_to_note(c, app.octave()) {
                                app.note_on(note, 100);
                            }
                        }
                        _ => {}
                    }
                } else if key.kind == KeyEventKind::Press && key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl + key combinations for effects
                    match key.code {
                        // Ctrl+1: Cycle distortion type
                        KeyCode::Char('1') => {
                            app.next_distortion_type();
                        }
                        // Ctrl+2: Decrease distortion drive
                        KeyCode::Char('2') => {
                            app.adjust_distortion_drive(-0.5);
                        }
                        // Ctrl+3: Increase distortion drive
                        KeyCode::Char('3') => {
                            app.adjust_distortion_drive(0.5);
                        }
                        // Ctrl+4: Toggle delay
                        KeyCode::Char('4') => {
                            app.toggle_delay();
                        }
                        // Ctrl+5: Decrease delay time
                        KeyCode::Char('5') => {
                            app.adjust_delay_time(-25.0);
                        }
                        // Ctrl+6: Increase delay time
                        KeyCode::Char('6') => {
                            app.adjust_delay_time(25.0);
                        }
                        // Ctrl+7: Toggle reverb
                        KeyCode::Char('7') => {
                            app.toggle_reverb();
                        }
                        // Ctrl+8: Decrease reverb mix
                        KeyCode::Char('8') => {
                            app.adjust_reverb_mix(-0.1);
                        }
                        // Ctrl+9: Increase reverb mix
                        KeyCode::Char('9') => {
                            app.adjust_reverb_mix(0.1);
                        }
                        _ => {}
                    }
                } else if key.kind == KeyEventKind::Release {
                    // Handle note off on key release
                    if let KeyCode::Char(c) = key.code {
                        if let Some(note) = input::keyboard::char_to_note(c, app.octave()) {
                            app.note_off(note);
                        }
                    }
                }
            }
        }

        // Update app state (animations, audio sync, etc.)
        app.tick();
    }
}
