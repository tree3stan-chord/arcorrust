//! arcorrust - Terminal-based virtual instrument playground
//!
//! A Rust TUI synthesizer with real-time audio synthesis.

mod app;
mod audio;
mod input;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new()?;
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
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
                        // Tab cycles waveform
                        KeyCode::Tab => {
                            app.next_waveform();
                        }
                        // Backtab (Shift+Tab) cycles waveform backwards
                        KeyCode::BackTab => {
                            app.prev_waveform();
                        }
                        // All other characters go to the musical keyboard
                        KeyCode::Char(c) => {
                            if let Some(note) = input::keyboard::char_to_note(c, app.octave()) {
                                app.note_on(note, 100);
                            }
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
