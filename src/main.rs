//! arcorrust - Terminal-based virtual instrument playground
//!
//! A Rust TUI synthesizer with real-time audio synthesis.

mod app;
mod arpeggiator;
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
                    // Check for Ctrl+key combinations first
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
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
                            // Ctrl+0: Adjust reverb decay down
                            KeyCode::Char('0') => {
                                app.adjust_reverb_decay(-0.1);
                            }
                            // Ctrl+-: Adjust reverb decay up
                            KeyCode::Char('-') => {
                                app.adjust_reverb_decay(0.1);
                            }
                            _ => {}
                        }
                    } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                        // Shift+key combinations - currently none
                        match key.code {
                            _ => {}
                        }
                    } else {
                        // Check which modal we're in for special handling
                        let in_lfo_modal = app.modal_panel() == app::ModalPanel::LFO;
                        let in_perf_modal = app.modal_panel() == app::ModalPanel::Performance;
                        let in_effects_modal = app.modal_panel() == app::ModalPanel::Effects;
                        let in_mod_modal = app.modal_panel() == app::ModalPanel::Modulation;

                        match key.code {
                            // Esc = close modal or quit
                            KeyCode::Esc => {
                                if app.modal_is_open() {
                                    app.close_modal();
                                } else {
                                    return Ok(());
                                }
                            }
                            // Space = LFO toggle in LFO modal, otherwise panic
                            KeyCode::Char(' ') => {
                                if in_lfo_modal {
                                    app.toggle_lfo();
                                } else {
                                    app.panic();
                                }
                            }
                            // Arrow keys - modal controls or normal
                            KeyCode::Up => {
                                if in_lfo_modal {
                                    app.adjust_lfo_rate(0.5);
                                } else if in_perf_modal {
                                    app.adjust_portamento_time(25.0);
                                    app.adjust_arpeggiator_bpm(5.0);
                                } else if in_effects_modal {
                                    app.adjust_bitcrusher_bits(1);
                                } else if in_mod_modal {
                                    app.adjust_ring_mod_freq(20.0);
                                    app.adjust_fm_ratio(0.25);
                                } else {
                                    app.adjust_volume(0.05);
                                }
                            }
                            KeyCode::Down => {
                                if in_lfo_modal {
                                    app.adjust_lfo_rate(-0.5);
                                } else if in_perf_modal {
                                    app.adjust_portamento_time(-25.0);
                                    app.adjust_arpeggiator_bpm(-5.0);
                                } else if in_effects_modal {
                                    app.adjust_bitcrusher_bits(-1);
                                } else if in_mod_modal {
                                    app.adjust_ring_mod_freq(-20.0);
                                    app.adjust_fm_ratio(-0.25);
                                } else {
                                    app.adjust_volume(-0.05);
                                }
                            }
                            KeyCode::Left => {
                                if in_lfo_modal {
                                    app.adjust_lfo_depth(-0.1);
                                } else if in_perf_modal {
                                    app.adjust_noise_level(-0.1);
                                    app.adjust_arpeggiator_gate(-0.1);
                                } else if in_effects_modal {
                                    app.adjust_bitcrusher_rate_div(-1);
                                } else if in_mod_modal {
                                    app.adjust_fm_amount(-0.1);
                                } else {
                                    app.octave_down();
                                }
                            }
                            KeyCode::Right => {
                                if in_lfo_modal {
                                    app.adjust_lfo_depth(0.1);
                                } else if in_perf_modal {
                                    app.adjust_noise_level(0.1);
                                    app.adjust_arpeggiator_gate(0.1);
                                } else if in_effects_modal {
                                    app.adjust_bitcrusher_rate_div(1);
                                } else if in_mod_modal {
                                    app.adjust_fm_amount(0.1);
                                } else {
                                    app.octave_up();
                                }
                            }
                            // Tab - cycle through modals
                            KeyCode::Tab => {
                                app.next_modal();
                            }
                            // Backtab (Shift+Tab) - cycle modals backwards
                            KeyCode::BackTab => {
                                app.prev_modal();
                            }
                            // LFO modal controls
                            KeyCode::Char('1') if in_lfo_modal => {
                                app.next_lfo_waveform();
                            }
                            // LFO destination toggles (only when in LFO modal)
                            KeyCode::Char('p') | KeyCode::Char('P') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::Pitch);
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::FilterCutoff);
                            }
                            KeyCode::Char('v') | KeyCode::Char('V') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::Volume);
                            }
                            KeyCode::Char('w') | KeyCode::Char('W') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::PulseWidth);
                            }
                            KeyCode::Char('o') | KeyCode::Char('O') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::Osc2Pitch);
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') if in_lfo_modal => {
                                app.toggle_lfo_destination(audio::LFODestination::Pan);
                            }
                            // Performance modal controls
                            KeyCode::Char('g') | KeyCode::Char('G') if in_perf_modal => {
                                app.next_portamento_mode();
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') if in_perf_modal => {
                                app.toggle_noise();
                            }
                            KeyCode::Char('t') | KeyCode::Char('T') if in_perf_modal => {
                                app.next_noise_type();
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') if in_perf_modal => {
                                app.toggle_arpeggiator();
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') if in_perf_modal => {
                                app.next_arpeggiator_division();
                            }
                            KeyCode::Char('p') | KeyCode::Char('P') if in_perf_modal => {
                                app.next_arpeggiator_pattern();
                            }
                            KeyCode::Char('o') | KeyCode::Char('O') if in_perf_modal => {
                                app.adjust_arpeggiator_octaves(1);
                            }
                            // Effects modal controls
                            KeyCode::Char('c') | KeyCode::Char('C') if in_effects_modal => {
                                app.toggle_chorus();
                            }
                            KeyCode::Char('p') | KeyCode::Char('P') if in_effects_modal => {
                                app.toggle_phaser();
                            }
                            KeyCode::Char('b') | KeyCode::Char('B') if in_effects_modal => {
                                app.toggle_bitcrusher();
                            }
                            // Modulation modal controls
                            KeyCode::Char('r') | KeyCode::Char('R') if in_mod_modal => {
                                app.toggle_ring_mod();
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') if in_mod_modal => {
                                app.toggle_fm();
                            }
                            // F1 = toggle recording
                            KeyCode::F(1) => {
                                if let Some(path) = app.toggle_recording() {
                                    log::info!("Recording saved to: {:?}", path);
                                }
                            }
                            // PageUp/PageDown for preset navigation
                            KeyCode::PageUp => {
                                app.prev_preset();
                            }
                            KeyCode::PageDown => {
                                app.next_preset();
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
                            // Insert toggles layout mode
                            KeyCode::Insert => {
                                app.toggle_layout();
                            }
                            // Home cycles visualization mode
                            KeyCode::Home => {
                                app.next_viz_mode();
                            }
                            // End cycles osc1 waveform (outside modals)
                            KeyCode::End => {
                                app.next_osc1_waveform();
                            }
                            // All other characters go to the musical keyboard
                            KeyCode::Char(c) => {
                                if let Some(note) = input::keyboard::char_to_note(c, app.octave()) {
                                    app.note_on(note, 100);
                                }
                            }
                            _ => {}
                        }
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
