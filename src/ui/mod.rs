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

    // Preset
    let preset = Paragraph::new(" Init")
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Preset ")
                .title_style(Style::default().fg(Color::White)),
        );
    frame.render_widget(preset, header_chunks[1]);

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
    frame.render_widget(volume, header_chunks[2]);
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
    let waveform = app.waveform();
    let detune = app.detune();
    let detune_str = if detune == 0.0 {
        "0".to_string()
    } else {
        format!("{:+.0}", detune)
    };

    let osc_text = vec![
        Line::from(vec![
            Span::styled(" Wave:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(waveform.name(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" (Tab)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Detune: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} cents", detune_str), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" OSC2:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Off", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled(" Mix:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("100%", Style::default().fg(Color::White)),
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
    let filter_text = vec![
        Line::from(vec![
            Span::styled(" Type:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Lowpass", Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::styled(" Cutoff: ", Style::default().fg(Color::DarkGray)),
            Span::styled("8000 Hz", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Reso:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("2.0", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Env:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("0%", Style::default().fg(Color::White)),
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

    // Envelope section
    let env_text = vec![
        Line::from(vec![
            Span::styled(" A: ", Style::default().fg(Color::DarkGray)),
            Span::styled("0.01s", Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled("D: ", Style::default().fg(Color::DarkGray)),
            Span::styled("0.10s", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" S: ", Style::default().fg(Color::DarkGray)),
            Span::styled("70%  ", Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled("R: ", Style::default().fg(Color::DarkGray)),
            Span::styled("0.30s", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" LFO:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("Off", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled(" Rate:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("4 Hz", Style::default().fg(Color::White)),
        ]),
    ];
    let envelope = Paragraph::new(env_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Envelope / LFO ")
            .title_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(envelope, chunks[2]);
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
