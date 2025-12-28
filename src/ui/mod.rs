//! Terminal user interface

pub mod widgets;

use crate::app::{App, LayoutMode, ModalPanel, VizMode};
use crate::input::keyboard::note_name;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use widgets::{PianoKeyboard, Visualizer};

/// Draw the main UI
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: header, main content, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Header
            Constraint::Min(10),     // Main content (sidebar + keyboard)
            Constraint::Length(3),   // Footer
        ])
        .split(area);

    draw_header(frame, main_chunks[0], app);
    draw_main_content(frame, main_chunks[1], app);
    draw_footer(frame, main_chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),  // Title
            Constraint::Length(14),  // Recording
            Constraint::Min(20),     // Preset
            Constraint::Length(20),  // Volume
        ])
        .split(area);

    // Title
    let title = Paragraph::new(" arcorrust")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" v{} ", env!("CARGO_PKG_VERSION")))
                .title_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(title, header_chunks[0]);

    // Recording indicator
    let (rec_text, rec_style) = if app.is_recording() {
        let duration = app.recording_duration();
        (
            format!(" ● {:.1}s", duration),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            " ○ F1 rec".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };
    let recording = Paragraph::new(rec_text)
        .style(rec_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(recording, header_chunks[1]);

    // Preset
    let preset_name = app.preset_name();
    let preset_idx = app.preset_index() + 1;
    let preset_count = app.preset_count();
    let modified_marker = if app.preset_modified() { "*" } else { "" };
    let preset_text = format!(" {}{} ({}/{})", preset_name, modified_marker, preset_idx, preset_count);
    let preset = Paragraph::new(preset_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" PgUp/Dn ")
                .title_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(preset, header_chunks[2]);

    // Volume gauge
    let volume_pct = (app.volume() * 100.0) as u16;
    let volume = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" ↑↓ ")
                .title_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .ratio(app.volume() as f64)
        .label(format!("{}%", volume_pct));
    frame.render_widget(volume, header_chunks[3]);
}

fn draw_main_content(frame: &mut Frame, area: Rect, app: &App) {
    match app.layout_mode() {
        LayoutMode::Sidebar => draw_sidebar_layout(frame, area, app),
        LayoutMode::Wide => draw_wide_layout(frame, area, app),
    }
}

fn draw_sidebar_layout(frame: &mut Frame, area: Rect, app: &App) {
    // Split into sidebar and keyboard area
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(32),  // Sidebar (controls)
            Constraint::Min(40),     // Keyboard area
        ])
        .split(area);

    draw_sidebar(frame, chunks[0], app);

    // In sidebar mode, modal slides up from bottom, keyboard shrinks
    let modal_panel = app.modal_panel();
    if modal_panel.is_open() {
        let keyboard_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),      // Keyboard (shrinks)
                Constraint::Length(8),   // Modal panel
            ])
            .split(chunks[1]);

        draw_keyboard_area(frame, keyboard_chunks[0], app);
        draw_modal(frame, keyboard_chunks[1], app, modal_panel);
    } else {
        draw_keyboard_area(frame, chunks[1], app);
    }
}

fn draw_wide_layout(frame: &mut Frame, area: Rect, app: &App) {
    let modal_panel = app.modal_panel();

    // In wide mode, modal grows from top, panels compress
    let rows = if modal_panel.is_open() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),   // Modal panel
                Constraint::Length(6),   // Control panels row (compressed)
                Constraint::Min(8),      // Keyboard
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),   // Control panels row
                Constraint::Min(8),      // Keyboard
            ])
            .split(area)
    };

    if modal_panel.is_open() {
        draw_modal(frame, rows[0], app, modal_panel);

        // Compressed control panels
        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(rows[1]);

        draw_oscillator_panel(frame, panels[0], app);
        draw_filter_panel(frame, panels[1], app);
        draw_effects_panel(frame, panels[2], app);
        draw_keyboard_area(frame, rows[2], app);
    } else {
        // Top row: three roughly square panels
        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(rows[0]);

        draw_oscillator_panel(frame, panels[0], app);
        draw_filter_panel(frame, panels[1], app);
        draw_effects_panel(frame, panels[2], app);
        draw_keyboard_area(frame, rows[1], app);
    }
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    // Stack controls vertically
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),   // Oscillator
            Constraint::Length(7),   // Filter
            Constraint::Length(6),   // Effects
            Constraint::Min(1),      // Spacer
        ])
        .split(area);

    // Oscillator section
    let osc1_wave = app.osc1_waveform();
    let osc1_detune = app.osc1_detune();
    let osc2_enabled = app.osc2_enabled();
    let osc2_wave = app.osc2_waveform();
    let mix = app.osc_mix();

    let osc2_status = if osc2_enabled {
        Span::styled(
            osc2_wave.name(),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let mix_pct = ((1.0 - mix) * 100.0) as u8;

    let osc_text = vec![
        Line::from(vec![
            Span::styled(" OSC1: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc1_wave.name(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {:+.0}¢", osc1_detune), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" OSC2: ", Style::default().fg(Color::DarkGray)),
            osc2_status,
            Span::styled(format!(" Mix {}:{}", mix_pct, 100 - mix_pct), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Tab", Style::default().fg(Color::Yellow)),
            Span::styled(" wave  ", Style::default().fg(Color::DarkGray)),
            Span::styled("F2", Style::default().fg(Color::Yellow)),
            Span::styled(" osc2  ", Style::default().fg(Color::DarkGray)),
            Span::styled("F3", Style::default().fg(Color::Yellow)),
            Span::styled(" wave2", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let osc = Paragraph::new(osc_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Oscillator ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(osc, chunks[0]);

    // Filter section
    let filter_enabled = app.filter_enabled();
    let filter_type = app.filter_type();
    let filter_cutoff = app.filter_cutoff();
    let filter_resonance = app.filter_resonance();
    let filter_env_amount = app.filter_env_amount();

    let filter_status = if filter_enabled {
        Span::styled(
            filter_type.name(),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let cutoff_str = if filter_cutoff >= 1000.0 {
        format!("{:.1}k", filter_cutoff / 1000.0)
    } else {
        format!("{:.0}", filter_cutoff)
    };

    let env_pct = (filter_env_amount * 100.0) as u8;

    let filter_text = vec![
        Line::from(vec![
            Span::styled(" Type: ", Style::default().fg(Color::DarkGray)),
            filter_status,
            Span::styled(format!("  Cut: {}Hz", cutoff_str), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Reso: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}", filter_resonance), Style::default().fg(Color::White)),
            Span::styled(format!("  Env: {}%", env_pct), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" F4", Style::default().fg(Color::Yellow)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("5", Style::default().fg(Color::Yellow)),
            Span::styled(" type ", Style::default().fg(Color::DarkGray)),
            Span::styled("F6-9", Style::default().fg(Color::Yellow)),
            Span::styled(" cut/res ", Style::default().fg(Color::DarkGray)),
            Span::styled("F10/11", Style::default().fg(Color::Yellow)),
        ]),
    ];
    let filter = Paragraph::new(filter_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Filter ")
            .title_style(Style::default().fg(Color::Magenta)),
    );
    frame.render_widget(filter, chunks[1]);

    // Effects section
    let dist_enabled = app.distortion_enabled();
    let dist_type = app.distortion_type_name();
    let delay_enabled = app.delay_enabled();
    let delay_time = app.delay_time_ms();
    let reverb_enabled = app.reverb_enabled();
    let reverb_mix = app.reverb_mix();

    let dist_status = if dist_enabled {
        Span::styled(dist_type, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let delay_status = if delay_enabled {
        Span::styled(format!("{:.0}ms", delay_time), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let reverb_status = if reverb_enabled {
        Span::styled(format!("{:.0}%", reverb_mix * 100.0), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let fx_text = vec![
        Line::from(vec![
            Span::styled(" Dist: ", Style::default().fg(Color::DarkGray)),
            dist_status,
            Span::styled("  Dly: ", Style::default().fg(Color::DarkGray)),
            delay_status,
            Span::styled("  Rev: ", Style::default().fg(Color::DarkGray)),
            reverb_status,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" F12", Style::default().fg(Color::Yellow)),
            Span::styled(" dist  ", Style::default().fg(Color::DarkGray)),
            Span::styled("^4", Style::default().fg(Color::Yellow)),
            Span::styled(" dly  ", Style::default().fg(Color::DarkGray)),
            Span::styled("^7", Style::default().fg(Color::Yellow)),
            Span::styled(" rev", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let effects = Paragraph::new(fx_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Effects ")
            .title_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(effects, chunks[2]);
}

fn draw_keyboard_area(frame: &mut Frame, area: Rect, app: &App) {
    let held_notes = app.held_notes();
    let octave = app.octave();
    let viz_mode = app.viz_mode();

    // Split area for keyboard and visualizer
    let has_viz = viz_mode != VizMode::Off;
    let main_chunks = if has_viz {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(12),     // Keyboard area
                Constraint::Length(10),  // Visualizer
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area)
    };

    // Draw keyboard section
    let keyboard_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Keyboard ")
        .title_style(Style::default().fg(Color::White));

    let inner = keyboard_block.inner(main_chunks[0]);
    frame.render_widget(keyboard_block, main_chunks[0]);

    // Split inner area: info bar, keyboard, key hints
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // Playing notes + octave
            Constraint::Min(8),      // Piano keyboard
            Constraint::Length(2),   // Key hints
        ])
        .split(inner);

    // Playing notes and octave info
    let notes_str = if held_notes.is_empty() {
        Span::styled("---", Style::default().fg(Color::DarkGray))
    } else {
        let mut sorted_notes: Vec<_> = held_notes.iter().copied().collect();
        sorted_notes.sort();
        Span::styled(
            sorted_notes
                .iter()
                .map(|&n| note_name(n))
                .collect::<Vec<_>>()
                .join(" "),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    };

    let info = Paragraph::new(Line::from(vec![
        Span::styled(" Playing: ", Style::default().fg(Color::DarkGray)),
        notes_str,
        Span::styled("    Octave: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("C{}", octave),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (←→)", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(info, chunks[0]);

    // Calculate how many octaves can fit
    let available_width = chunks[1].width as usize;
    let key_width = 6; // WHITE_KEY_WIDTH
    let keys_per_octave = 7; // white keys per octave
    let max_octaves = (available_width / (key_width * keys_per_octave)).max(1).min(4);

    // Center the keyboard horizontally
    let keyboard_width = (max_octaves * keys_per_octave * key_width) as u16;
    let x_offset = (chunks[1].width.saturating_sub(keyboard_width)) / 2;

    let keyboard_area = Rect {
        x: chunks[1].x + x_offset,
        y: chunks[1].y,
        width: keyboard_width.min(chunks[1].width),
        height: chunks[1].height,
    };

    // Piano keyboard widget
    let piano = PianoKeyboard::new(held_notes, octave).octaves(max_octaves as u16);
    frame.render_widget(piano, keyboard_area);

    // Key mapping hints
    let hints = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" Lower: ", Style::default().fg(Color::DarkGray)),
            Span::styled("Z S X D C V G B H N J M", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Upper: ", Style::default().fg(Color::DarkGray)),
            Span::styled("Q 2 W 3 E R 5 T 6 Y 7 U", Style::default().fg(Color::White)),
        ]),
    ]);
    frame.render_widget(hints, chunks[2]);

    // Draw visualizer if enabled
    if has_viz {
        let samples = app.get_viz_samples();
        let sample_rate = app.sample_rate();
        let viz_title = format!(" {} ", viz_mode.name());
        let visualizer = Visualizer::new(&samples, sample_rate, viz_mode).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(viz_title)
                .title_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(visualizer, main_chunks[1]);
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(40),     // Controls help
            Constraint::Length(25),  // Status
        ])
        .split(area);

    // Controls help
    let layout_name = app.layout_mode().name();
    let viz_name = app.viz_mode().name();
    let modal_panel = app.modal_panel();
    let modal_indicator = if modal_panel.is_open() {
        Span::styled(
            format!(" [{}] ", modal_panel.name()),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" Tab", Style::default().fg(Color::Yellow))
    };

    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::styled(" panic  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Ins", Style::default().fg(Color::Yellow)),
        Span::styled(format!(" {} ", layout_name), Style::default().fg(Color::Cyan)),
        Span::styled("Home", Style::default().fg(Color::Yellow)),
        Span::styled(format!(" {} ", viz_name), Style::default().fg(Color::Magenta)),
        modal_indicator,
        if !modal_panel.is_open() {
            Span::styled(" modal", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled("", Style::default())
        },
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(controls, footer_chunks[0]);

    // Status
    let voice_color = if app.voice_count() > 0 {
        Color::Green
    } else {
        Color::DarkGray
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(" Voices: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/20", app.voice_count()),
            Style::default().fg(voice_color),
        ),
        Span::styled("  CPU: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1}%", app.cpu_usage()),
            Style::default().fg(Color::White),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(status, footer_chunks[1]);
}

// === Dashboard panel layouts (square-ish panels) ===

fn draw_oscillator_panel(frame: &mut Frame, area: Rect, app: &App) {
    let osc1_wave = app.osc1_waveform();
    let osc1_detune = app.osc1_detune();
    let osc2_enabled = app.osc2_enabled();
    let osc2_wave = app.osc2_waveform();
    let mix = app.osc_mix();

    let osc2_status = if osc2_enabled {
        Span::styled(
            osc2_wave.name(),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let mix_pct = ((1.0 - mix) * 100.0) as u8;

    let osc_text = vec![
        Line::from(vec![
            Span::styled(" OSC1: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc1_wave.name(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {:+.0}¢", osc1_detune), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" OSC2: ", Style::default().fg(Color::DarkGray)),
            osc2_status,
        ]),
        Line::from(vec![
            Span::styled(" Mix:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}:{}", mix_pct, 100 - mix_pct), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Tab", Style::default().fg(Color::Yellow)),
            Span::styled(" wave ", Style::default().fg(Color::DarkGray)),
            Span::styled("F2", Style::default().fg(Color::Yellow)),
            Span::styled(" osc2 ", Style::default().fg(Color::DarkGray)),
            Span::styled("F3", Style::default().fg(Color::Yellow)),
            Span::styled(" w2", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let osc = Paragraph::new(osc_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Oscillator ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(osc, area);
}

fn draw_filter_panel(frame: &mut Frame, area: Rect, app: &App) {
    let filter_enabled = app.filter_enabled();
    let filter_type = app.filter_type();
    let filter_cutoff = app.filter_cutoff();
    let filter_resonance = app.filter_resonance();
    let filter_env_amount = app.filter_env_amount();

    let filter_status = if filter_enabled {
        Span::styled(
            filter_type.name(),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let cutoff_str = if filter_cutoff >= 1000.0 {
        format!("{:.1}k", filter_cutoff / 1000.0)
    } else {
        format!("{:.0}", filter_cutoff)
    };

    let env_pct = (filter_env_amount * 100.0) as u8;

    let filter_text = vec![
        Line::from(vec![
            Span::styled(" Type: ", Style::default().fg(Color::DarkGray)),
            filter_status,
        ]),
        Line::from(vec![
            Span::styled(" Cut:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}Hz", cutoff_str), Style::default().fg(Color::White)),
            Span::styled(format!("  Res: {:.1}", filter_resonance), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Env:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}%", env_pct), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" F4", Style::default().fg(Color::Yellow)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("5", Style::default().fg(Color::Yellow)),
            Span::styled(" ", Style::default().fg(Color::DarkGray)),
            Span::styled("F6-9", Style::default().fg(Color::Yellow)),
            Span::styled(" ", Style::default().fg(Color::DarkGray)),
            Span::styled("F10/11", Style::default().fg(Color::Yellow)),
        ]),
    ];
    let filter = Paragraph::new(filter_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Filter ")
            .title_style(Style::default().fg(Color::Magenta)),
    );
    frame.render_widget(filter, area);
}

fn draw_effects_panel(frame: &mut Frame, area: Rect, app: &App) {
    let dist_enabled = app.distortion_enabled();
    let dist_type = app.distortion_type_name();
    let delay_enabled = app.delay_enabled();
    let delay_time = app.delay_time_ms();
    let reverb_enabled = app.reverb_enabled();
    let reverb_mix = app.reverb_mix();

    let dist_status = if dist_enabled {
        Span::styled(dist_type, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let delay_status = if delay_enabled {
        Span::styled(format!("{:.0}ms", delay_time), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let reverb_status = if reverb_enabled {
        Span::styled(format!("{:.0}%", reverb_mix * 100.0), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let fx_text = vec![
        Line::from(vec![
            Span::styled(" Dist:  ", Style::default().fg(Color::DarkGray)),
            dist_status,
        ]),
        Line::from(vec![
            Span::styled(" Delay: ", Style::default().fg(Color::DarkGray)),
            delay_status,
        ]),
        Line::from(vec![
            Span::styled(" Reverb:", Style::default().fg(Color::DarkGray)),
            reverb_status,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" F12", Style::default().fg(Color::Yellow)),
            Span::styled(" ", Style::default().fg(Color::DarkGray)),
            Span::styled("^4", Style::default().fg(Color::Yellow)),
            Span::styled(" dly ", Style::default().fg(Color::DarkGray)),
            Span::styled("^7", Style::default().fg(Color::Yellow)),
            Span::styled(" rev", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let effects = Paragraph::new(fx_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Effects ")
            .title_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(effects, area);
}

// === Modal Panels ===

fn draw_modal(frame: &mut Frame, area: Rect, app: &App, modal: ModalPanel) {
    match modal {
        ModalPanel::None => {}
        ModalPanel::LFO => draw_lfo_modal(frame, area, app),
        ModalPanel::Modulation => draw_modulation_modal(frame, area, app),
        ModalPanel::Effects => draw_effects_modal(frame, area, app),
        ModalPanel::Performance => draw_performance_modal(frame, area, app),
    }
}

fn draw_lfo_modal(frame: &mut Frame, area: Rect, app: &App) {
    use crate::audio::LFODestination;

    let enabled = app.lfo_enabled();
    let waveform = app.lfo_waveform();
    let rate = app.lfo_rate();
    let depth = app.lfo_depth();

    // Build destination checkboxes
    let dest_pitch = if app.lfo_has_destination(LFODestination::Pitch) { "[x]" } else { "[ ]" };
    let dest_filter = if app.lfo_has_destination(LFODestination::FilterCutoff) { "[x]" } else { "[ ]" };
    let dest_volume = if app.lfo_has_destination(LFODestination::Volume) { "[x]" } else { "[ ]" };
    let dest_pwm = if app.lfo_has_destination(LFODestination::PulseWidth) { "[x]" } else { "[ ]" };
    let dest_osc2 = if app.lfo_has_destination(LFODestination::Osc2Pitch) { "[x]" } else { "[ ]" };
    let dest_pan = if app.lfo_has_destination(LFODestination::Pan) { "[x]" } else { "[ ]" };

    let status_style = if enabled {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    };

    let content = vec![
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(if enabled { "ON" } else { "OFF" }, status_style),
            Span::styled("  Wave: ", Style::default().fg(Color::DarkGray)),
            Span::styled(waveform.name(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1} Hz", rate), Style::default().fg(Color::White)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", depth * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Routing: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}P ", dest_pitch), if app.lfo_has_destination(LFODestination::Pitch) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("{}F ", dest_filter), if app.lfo_has_destination(LFODestination::FilterCutoff) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("{}V ", dest_volume), if app.lfo_has_destination(LFODestination::Volume) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("{}W ", dest_pwm), if app.lfo_has_destination(LFODestination::PulseWidth) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("{}O ", dest_osc2), if app.lfo_has_destination(LFODestination::Osc2Pitch) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("{}A", dest_pan), if app.lfo_has_destination(LFODestination::Pan) { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) }),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Space", Style::default().fg(Color::Yellow)),
            Span::styled(" on/off ", Style::default().fg(Color::DarkGray)),
            Span::styled("1", Style::default().fg(Color::Yellow)),
            Span::styled(" wave ", Style::default().fg(Color::DarkGray)),
            Span::styled("^/v", Style::default().fg(Color::Yellow)),
            Span::styled(" rate ", Style::default().fg(Color::DarkGray)),
            Span::styled("</>", Style::default().fg(Color::Yellow)),
            Span::styled(" depth ", Style::default().fg(Color::DarkGray)),
            Span::styled("P/F/V/W/O/A", Style::default().fg(Color::Yellow)),
            Span::styled(" route", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" LFO ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_modulation_modal(frame: &mut Frame, area: Rect, app: &App) {
    // Ring Mod
    let ring_enabled = app.ring_mod_enabled();
    let ring_freq = app.ring_mod_freq();
    let ring_mix = app.ring_mod_mix();

    let ring_status = if ring_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    // FM Synthesis
    let fm_enabled = app.fm_enabled();
    let fm_amount = app.fm_amount();
    let fm_ratio = app.fm_ratio();

    let fm_status = if fm_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let content = vec![
        Line::from(vec![
            Span::styled(" Ring Mod: ", Style::default().fg(Color::DarkGray)),
            ring_status,
            Span::styled("  Freq: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0} Hz", ring_freq), Style::default().fg(Color::Cyan)),
            Span::styled("  Mix: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", ring_mix * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" FM:       ", Style::default().fg(Color::DarkGray)),
            fm_status,
            Span::styled("  Amount: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.2}", fm_amount), Style::default().fg(Color::Cyan)),
            Span::styled("  Ratio: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.2}", fm_ratio), Style::default().fg(Color::White)),
            Span::styled("  (OSC2→OSC1)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" R", Style::default().fg(Color::Yellow)),
            Span::styled(" ring ", Style::default().fg(Color::DarkGray)),
            Span::styled("F", Style::default().fg(Color::Yellow)),
            Span::styled(" fm ", Style::default().fg(Color::DarkGray)),
            Span::styled("</>", Style::default().fg(Color::Yellow)),
            Span::styled(" amount ", Style::default().fg(Color::DarkGray)),
            Span::styled("^/v", Style::default().fg(Color::Yellow)),
            Span::styled(" ratio/freq ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" close", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Modulation ")
            .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_effects_modal(frame: &mut Frame, area: Rect, app: &App) {
    // Chorus
    let chorus_enabled = app.chorus_enabled();
    let chorus_rate = app.chorus_rate();
    let chorus_depth = app.chorus_depth();
    let chorus_voices = app.chorus_voices();

    let chorus_status = if chorus_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    // Phaser
    let phaser_enabled = app.phaser_enabled();
    let phaser_rate = app.phaser_rate();
    let phaser_depth = app.phaser_depth();
    let phaser_stages = app.phaser_stages();

    let phaser_status = if phaser_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    // Bitcrusher
    let crush_enabled = app.bitcrusher_enabled();
    let crush_bits = app.bitcrusher_bits();
    let crush_rate_div = app.bitcrusher_rate_div();

    let crush_status = if crush_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let content = vec![
        Line::from(vec![
            Span::styled(" Chorus:    ", Style::default().fg(Color::DarkGray)),
            chorus_status,
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}Hz", chorus_rate), Style::default().fg(Color::Cyan)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}ms", chorus_depth), Style::default().fg(Color::White)),
            Span::styled("  Voices: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", chorus_voices), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Phaser:    ", Style::default().fg(Color::DarkGray)),
            phaser_status,
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}Hz", phaser_rate), Style::default().fg(Color::Cyan)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", phaser_depth * 100.0), Style::default().fg(Color::White)),
            Span::styled("  Stages: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", phaser_stages), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Bitcrusher:", Style::default().fg(Color::DarkGray)),
            crush_status,
            Span::styled("  Bits: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", crush_bits), Style::default().fg(Color::Cyan)),
            Span::styled("  SR Div: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}x", crush_rate_div), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" C", Style::default().fg(Color::Yellow)),
            Span::styled(" chorus ", Style::default().fg(Color::DarkGray)),
            Span::styled("P", Style::default().fg(Color::Yellow)),
            Span::styled(" phaser ", Style::default().fg(Color::DarkGray)),
            Span::styled("B", Style::default().fg(Color::Yellow)),
            Span::styled(" crush ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" close", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Effects (Extended) ")
            .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_performance_modal(frame: &mut Frame, area: Rect, app: &App) {
    use crate::audio::PortamentoMode;

    // Portamento
    let portamento_mode = app.portamento_mode();
    let portamento_time = app.portamento_time();
    let porta_enabled = portamento_mode != PortamentoMode::Off;

    let porta_status = if porta_enabled {
        Span::styled(portamento_mode.name(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    // Noise
    let noise_enabled = app.noise_enabled();
    let noise_type = app.noise_type();
    let noise_level = app.noise_level();

    let noise_status = if noise_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    // Arpeggiator
    let arp_enabled = app.arpeggiator_enabled();
    let arp_bpm = app.arpeggiator_bpm();
    let arp_pattern = app.arpeggiator_pattern();
    let arp_division = app.arpeggiator_division();
    let arp_octaves = app.arpeggiator_octaves();
    let arp_gate = app.arpeggiator_gate();

    let arp_status = if arp_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let content = vec![
        Line::from(vec![
            Span::styled(" Portamento: ", Style::default().fg(Color::DarkGray)),
            porta_status,
            Span::styled("  Time: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}ms", portamento_time), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Noise:      ", Style::default().fg(Color::DarkGray)),
            noise_status,
            Span::styled("  Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled(noise_type.name(), Style::default().fg(Color::Cyan)),
            Span::styled("  Level: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", noise_level * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Arpeggiator:", Style::default().fg(Color::DarkGray)),
            arp_status,
            Span::styled("  BPM: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", arp_bpm), Style::default().fg(Color::Cyan)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(arp_division.name(), Style::default().fg(Color::White)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(arp_pattern.name(), Style::default().fg(Color::Magenta)),
            Span::styled("  Oct: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", arp_octaves), Style::default().fg(Color::White)),
            Span::styled("  Gate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", arp_gate * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" G", Style::default().fg(Color::Yellow)),
            Span::styled(" glide ", Style::default().fg(Color::DarkGray)),
            Span::styled("N", Style::default().fg(Color::Yellow)),
            Span::styled(" noise ", Style::default().fg(Color::DarkGray)),
            Span::styled("A", Style::default().fg(Color::Yellow)),
            Span::styled(" arp ", Style::default().fg(Color::DarkGray)),
            Span::styled("D", Style::default().fg(Color::Yellow)),
            Span::styled(" div ", Style::default().fg(Color::DarkGray)),
            Span::styled("P", Style::default().fg(Color::Yellow)),
            Span::styled(" pattern ", Style::default().fg(Color::DarkGray)),
            Span::styled("O", Style::default().fg(Color::Yellow)),
            Span::styled(" oct", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(" Performance ")
            .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}
