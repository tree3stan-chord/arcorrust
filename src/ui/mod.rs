//! Terminal user interface

pub mod widgets;

use crate::app::App;
use crate::input::keyboard::note_name;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use widgets::PianoKeyboard;

/// Draw the main UI
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: header, synth controls, keyboard, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(8),  // Synth controls (Osc, Filter, Env)
            Constraint::Min(8),     // Keyboard display
            Constraint::Length(3),  // Footer/status
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    draw_synth_controls(frame, chunks[1], app);
    draw_keyboard(frame, chunks[2], app);
    draw_footer(frame, chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),  // Title
            Constraint::Length(16),  // Recording
            Constraint::Min(20),     // Preset
            Constraint::Length(25),  // Volume
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
            format!(" REC {:.1}s", duration),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            " F1 to rec".to_string(),
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
                .title(" Preset (PgUp/Dn) ")
                .title_style(Style::default().fg(Color::White)),
        );
    frame.render_widget(preset, header_chunks[2]);

    // Volume gauge
    let volume_pct = (app.volume() * 100.0) as u16;
    let volume = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Volume ")
                .title_style(Style::default().fg(Color::White)),
        )
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .ratio(app.volume() as f64)
        .label(format!("{}%", volume_pct));
    frame.render_widget(volume, header_chunks[3]);
}

fn draw_synth_controls(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Oscillator section - show actual values from app
    let osc1_wave = app.osc1_waveform();
    let osc1_detune = app.osc1_detune();
    let osc2_enabled = app.osc2_enabled();
    let osc2_wave = app.osc2_waveform();
    let osc2_detune = app.osc2_detune();
    let osc2_pitch = app.osc2_pitch();
    let mix = app.osc_mix();

    let osc2_status = if osc2_enabled {
        Span::styled(
            format!("{}", osc2_wave.name()),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("Off", Style::default().fg(Color::Red))
    };

    let mix_pct = ((1.0 - mix) * 100.0) as u8;

    let osc_text = vec![
        Line::from(vec![
            Span::styled(" OSC1: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc1_wave.name(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" (Tab)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Det:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:+.0}¢", osc1_detune), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" OSC2: ", Style::default().fg(Color::DarkGray)),
            osc2_status,
            Span::styled(" (F2/F3)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Mix:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}:{}", mix_pct, 100 - mix_pct), Style::default().fg(Color::White)),
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

    // Filter section - show actual values from app
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

    // Format cutoff nicely
    let cutoff_str = if filter_cutoff >= 1000.0 {
        format!("{:.1}k Hz", filter_cutoff / 1000.0)
    } else {
        format!("{:.0} Hz", filter_cutoff)
    };

    let env_pct = (filter_env_amount * 100.0) as u8;

    let filter_text = vec![
        Line::from(vec![
            Span::styled(" Type:   ", Style::default().fg(Color::DarkGray)),
            filter_status,
            Span::styled(" (F4/F5)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Cutoff: ", Style::default().fg(Color::DarkGray)),
            Span::styled(cutoff_str, Style::default().fg(Color::White)),
            Span::styled(" (F6/F7)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Reso:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}", filter_resonance), Style::default().fg(Color::White)),
            Span::styled(" (F8/F9)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Env:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}%", env_pct), Style::default().fg(Color::Yellow)),
            Span::styled(" (F10/11)", Style::default().fg(Color::DarkGray)),
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
    let dist_drive = app.distortion_drive();
    let delay_enabled = app.delay_enabled();
    let delay_time = app.delay_time_ms();
    let delay_fb = app.delay_feedback();
    let reverb_enabled = app.reverb_enabled();
    let reverb_mix = app.reverb_mix();

    let dist_status = if dist_enabled {
        Span::styled(dist_type, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let delay_status = if delay_enabled {
        Span::styled("On", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let reverb_status = if reverb_enabled {
        Span::styled("On", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Off", Style::default().fg(Color::DarkGray))
    };

    let fx_text = vec![
        Line::from(vec![
            Span::styled(" Dist: ", Style::default().fg(Color::DarkGray)),
            dist_status,
            Span::styled(format!(" {:.1}x", dist_drive), Style::default().fg(Color::White)),
            Span::styled(" (F12)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Dly:  ", Style::default().fg(Color::DarkGray)),
            delay_status,
            Span::styled(format!(" {:.0}ms {:.0}%", delay_time, delay_fb * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Verb: ", Style::default().fg(Color::DarkGray)),
            reverb_status,
            Span::styled(format!(" {:.0}%", reverb_mix * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" ^1-3 Dist ^4-6 Dly ^7-9 Verb", Style::default().fg(Color::DarkGray)),
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

fn draw_keyboard(frame: &mut Frame, area: Rect, app: &App) {
    let held_notes = app.held_notes();
    let octave = app.octave();

    // Split into header, piano, and info
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Playing + octave info
            Constraint::Min(6),     // Piano keyboard
            Constraint::Length(1),  // Help text
        ])
        .split(area);

    // Currently playing notes + octave
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

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Playing: ", Style::default().fg(Color::White)),
        notes_str,
        Span::styled("    Octave: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("C{}", octave),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (", Style::default().fg(Color::DarkGray)),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        Span::styled(" to shift)", Style::default().fg(Color::DarkGray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Keyboard ")
            .title_style(Style::default().fg(Color::White)),
    );
    frame.render_widget(header, chunks[0]);

    // Piano keyboard widget
    let piano = PianoKeyboard::new(held_notes, octave).octaves(2);
    frame.render_widget(piano, chunks[1]);

    // Help text
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" Lower row: ", Style::default().fg(Color::DarkGray)),
        Span::styled("Z S X D C V G B H N J M", Style::default().fg(Color::White)),
        Span::styled("    Upper row: ", Style::default().fg(Color::DarkGray)),
        Span::styled("Q 2 W 3 E R 5 T 6 Y 7 U", Style::default().fg(Color::White)),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help, chunks[2]);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(area);

    // Controls help
    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" Quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::styled(" Panic  ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::styled(" Vol  ", Style::default().fg(Color::DarkGray)),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        Span::styled(" Oct  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::styled(" Wave", Style::default().fg(Color::DarkGray)),
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
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Status ")
            .title_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(status, footer_chunks[1]);
}
