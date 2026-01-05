//! Terminal user interface

pub mod widgets;

use crate::app::{App, LayoutMode, ModalPanel, VizMode};
use crate::input::keyboard::note_name;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use widgets::{PianoKeyboard, Visualizer};

/// Draw the main UI
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: header, modal row, main content, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Header
            Constraint::Length(3),   // Modal row (compact panels)
            Constraint::Min(10),     // Main content (sidebar + keyboard)
            Constraint::Length(3),   // Footer
        ])
        .split(area);

    draw_header(frame, main_chunks[0], app);
    draw_modal_row(frame, main_chunks[1], app);
    draw_main_content(frame, main_chunks[2], app);
    draw_footer(frame, main_chunks[3], app);
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
    draw_keyboard_area(frame, chunks[1], app);
}

fn draw_wide_layout(frame: &mut Frame, area: Rect, app: &App) {
    // Wide mode: control panels row + keyboard area
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // Control panels row
            Constraint::Min(8),      // Keyboard area (includes visualizer + modal)
        ])
        .split(area);

    // Top row: three control panels
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

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let active_modal = app.modal_panel();

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

    draw_sidebar_oscillator(frame, chunks[0], app, active_modal == ModalPanel::Oscillator);
    draw_sidebar_filter(frame, chunks[1], app, active_modal == ModalPanel::Filter);
    draw_sidebar_effects(frame, chunks[2], app, active_modal == ModalPanel::EffectsBasic);
}

fn draw_sidebar_oscillator(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
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

    // Active panel gets brighter text
    let label_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let osc_text = vec![
        Line::from(vec![
            Span::styled(" OSC1: ", label_style),
            Span::styled(osc1_wave.name(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {:+.0}¢", osc1_detune), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" OSC2: ", label_style),
            osc2_status,
            Span::styled(format!(" Mix {}:{}", mix_pct, 100 - mix_pct), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" 1", Style::default().fg(Color::Yellow)),
            Span::styled(" wave  ", Style::default().fg(Color::DarkGray)),
            Span::styled("2", Style::default().fg(Color::Yellow)),
            Span::styled(" osc2  ", Style::default().fg(Color::DarkGray)),
            Span::styled("3", Style::default().fg(Color::Yellow)),
            Span::styled(" wave2", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let osc = Paragraph::new(osc_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Oscillator ")
            .title_style(title_style),
    );
    frame.render_widget(osc, area);
}

fn draw_sidebar_filter(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
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

    // Active panel gets brighter text
    let label_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let filter_text = vec![
        Line::from(vec![
            Span::styled(" Type: ", label_style),
            filter_status,
            Span::styled(format!("  Cut: {}Hz", cutoff_str), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Reso: ", label_style),
            Span::styled(format!("{:.1}", filter_resonance), Style::default().fg(Color::White)),
            Span::styled(format!("  Env: {}%", env_pct), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" 4", Style::default().fg(Color::Yellow)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("5", Style::default().fg(Color::Yellow)),
            Span::styled(" type ", Style::default().fg(Color::DarkGray)),
            Span::styled("^/v", Style::default().fg(Color::Yellow)),
            Span::styled(" cut ", Style::default().fg(Color::DarkGray)),
            Span::styled("</>", Style::default().fg(Color::Yellow)),
            Span::styled(" res", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if is_active { Color::Magenta } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Magenta)
    };

    let filter = Paragraph::new(filter_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Filter ")
            .title_style(title_style),
    );
    frame.render_widget(filter, area);
}

fn draw_sidebar_effects(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
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

    // Active panel gets brighter text
    let label_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let fx_text = vec![
        Line::from(vec![
            Span::styled(" Dist: ", label_style),
            dist_status,
            Span::styled("  Dly: ", label_style),
            delay_status,
            Span::styled("  Rev: ", label_style),
            reverb_status,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" D", Style::default().fg(Color::Yellow)),
            Span::styled(" dist  ", Style::default().fg(Color::DarkGray)),
            Span::styled("L", Style::default().fg(Color::Yellow)),
            Span::styled(" dly  ", Style::default().fg(Color::DarkGray)),
            Span::styled("R", Style::default().fg(Color::Yellow)),
            Span::styled(" rev", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let effects = Paragraph::new(fx_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Effects ")
            .title_style(title_style),
    );
    frame.render_widget(effects, area);
}

fn draw_keyboard_area(frame: &mut Frame, area: Rect, app: &App) {
    let held_notes = app.held_notes();
    let octave = app.octave();
    let viz_mode = app.viz_mode();
    let modal_panel = app.modal_panel();

    // Split area for keyboard, visualizer, and enlarged modal
    let has_viz = viz_mode != VizMode::Off;
    let main_chunks = if has_viz {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),     // Keyboard area
                Constraint::Length(8),   // Visualizer
                Constraint::Length(6),   // Enlarged modal
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),     // Keyboard area
                Constraint::Length(6),   // Enlarged modal
            ])
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
        // Draw enlarged modal
        draw_modal(frame, main_chunks[2], app, modal_panel);
    } else {
        // Draw enlarged modal (no visualizer)
        draw_modal(frame, main_chunks[1], app, modal_panel);
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

    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::styled(" panic  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Ins", Style::default().fg(Color::Yellow)),
        Span::styled(format!(" {} ", layout_name), Style::default().fg(Color::Cyan)),
        Span::styled("Home", Style::default().fg(Color::Yellow)),
        Span::styled(format!(" {} ", viz_name), Style::default().fg(Color::Magenta)),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::styled(format!(" [{}]", modal_panel.name()), Style::default().fg(Color::Cyan)),
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

// === Compact Modal Row ===

fn draw_modal_row(frame: &mut Frame, area: Rect, app: &App) {
    let active_modal = app.modal_panel();

    // Split into 5 equal columns
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(area);

    draw_compact_envelope(frame, panels[0], app, active_modal == ModalPanel::Envelope);
    draw_compact_lfo(frame, panels[1], app, active_modal == ModalPanel::LFO);
    draw_compact_modulation(frame, panels[2], app, active_modal == ModalPanel::Modulation);
    draw_compact_effects(frame, panels[3], app, active_modal == ModalPanel::EffectsExt);
    draw_compact_performance(frame, panels[4], app, active_modal == ModalPanel::Performance);
}

fn draw_compact_envelope(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
    let amp_a = app.amp_attack();
    let amp_s = app.amp_sustain();

    // Show compact ADSR summary: A and S are most distinctive
    let content = Line::from(vec![
        Span::styled("A:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.0}ms", amp_a * 1000.0), Style::default().fg(Color::Cyan)),
        Span::styled(" S:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.0}%", amp_s * 100.0), Style::default().fg(Color::Cyan)),
    ]);

    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let widget = Paragraph::new(content)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(" Env ")
                .title_style(title_style),
        );
    frame.render_widget(widget, area);
}

fn draw_compact_lfo(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
    let enabled = app.lfo_enabled();
    let waveform = app.lfo_waveform();

    let status = if enabled { "On" } else { "Off" };
    let status_color = if enabled { Color::Green } else { Color::Red };

    let content = Line::from(vec![
        Span::styled(status, Style::default().fg(status_color)),
        Span::styled(" ", Style::default()),
        Span::styled(waveform.name(), Style::default().fg(Color::Cyan)),
    ]);

    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" LFO ")
            .title_style(title_style),
    );
    frame.render_widget(widget, area);
}

fn draw_compact_modulation(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
    let ring_on = app.ring_mod_enabled();
    let fm_on = app.fm_enabled();

    let content = Line::from(vec![
        Span::styled("Ring:", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if ring_on { "On" } else { "Off" },
            Style::default().fg(if ring_on { Color::Green } else { Color::Red }),
        ),
        Span::styled(" FM:", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if fm_on { "On" } else { "Off" },
            Style::default().fg(if fm_on { Color::Green } else { Color::Red }),
        ),
    ]);

    let border_color = if is_active { Color::Magenta } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Mod ")
            .title_style(title_style),
    );
    frame.render_widget(widget, area);
}

fn draw_compact_effects(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
    let chorus_on = app.chorus_enabled();
    let phaser_on = app.phaser_enabled();
    let crush_on = app.bitcrusher_enabled();

    let content = Line::from(vec![
        Span::styled(
            "C",
            Style::default().fg(if chorus_on { Color::Green } else { Color::DarkGray }),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            "P",
            Style::default().fg(if phaser_on { Color::Green } else { Color::DarkGray }),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            "B",
            Style::default().fg(if crush_on { Color::Green } else { Color::DarkGray }),
        ),
    ]);

    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" FX+ ")
            .title_style(title_style),
    );
    frame.render_widget(widget, area);
}

fn draw_compact_performance(frame: &mut Frame, area: Rect, app: &App, is_active: bool) {
    let arp_on = app.arpeggiator_enabled();
    let noise_on = app.noise_enabled();
    let porta_on = app.portamento_mode() != crate::audio::PortamentoMode::Off;

    let content = Line::from(vec![
        Span::styled(
            "Arp",
            Style::default().fg(if arp_on { Color::Green } else { Color::DarkGray }),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            "N",
            Style::default().fg(if noise_on { Color::Green } else { Color::DarkGray }),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            "G",
            Style::default().fg(if porta_on { Color::Green } else { Color::DarkGray }),
        ),
    ]);

    let border_color = if is_active { Color::Green } else { Color::DarkGray };
    let title_style = if is_active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Perf ")
            .title_style(title_style),
    );
    frame.render_widget(widget, area);
}

// === Modal Panels (Enlarged) ===

fn draw_modal(frame: &mut Frame, area: Rect, app: &App, modal: ModalPanel) {
    let edit_mode = app.edit_mode();
    let param_idx = app.param_index();

    match modal {
        ModalPanel::Oscillator => draw_oscillator_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::Filter => draw_filter_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::Envelope => draw_envelope_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::EffectsBasic => draw_effects_basic_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::LFO => draw_lfo_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::Modulation => draw_modulation_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::EffectsExt => draw_effects_ext_modal(frame, area, app, edit_mode, param_idx),
        ModalPanel::Performance => draw_performance_modal(frame, area, app, edit_mode, param_idx),
    }
}

fn draw_oscillator_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    let osc1_wave = app.osc1_waveform();
    let osc1_detune = app.osc1_detune();
    let osc2_enabled = app.osc2_enabled();
    let osc2_wave = app.osc2_waveform();
    let mix = app.osc_mix();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let osc2_color = if osc2_enabled { Color::Green } else { Color::Red };
    let osc2_status = if osc2_enabled { "On" } else { "Off" };

    let mix_pct = ((1.0 - mix) * 100.0) as u8;

    // Params: 0=Wave1, 1=Osc2 on/off, 2=Wave2, 3=Mix
    let content = vec![
        Line::from(vec![
            Span::styled(" OSC1: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc1_wave.name(), param_style(0, Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  Detune: {:+.0}¢", osc1_detune), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" OSC2: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc2_status, param_style(1, osc2_color)),
            Span::styled("  Wave: ", Style::default().fg(Color::DarkGray)),
            Span::styled(osc2_wave.name(), param_style(2, Color::Cyan)),
            Span::styled("  Mix: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}:{}", mix_pct, 100 - mix_pct), param_style(3, Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Cyan };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Oscillator [EDIT] " } else { " Oscillator " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_filter_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    let filter_enabled = app.filter_enabled();
    let filter_type = app.filter_type();
    let filter_cutoff = app.filter_cutoff();
    let filter_resonance = app.filter_resonance();
    let filter_env_amount = app.filter_env_amount();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let enabled_color = if filter_enabled { Color::Green } else { Color::Red };
    let enabled_str = if filter_enabled { "On" } else { "Off" };

    let cutoff_str = if filter_cutoff >= 1000.0 {
        format!("{:.1}kHz", filter_cutoff / 1000.0)
    } else {
        format!("{:.0}Hz", filter_cutoff)
    };

    // Params: 0=On/Off, 1=Type, 2=Cutoff, 3=Resonance, 4=EnvAmt
    let content = vec![
        Line::from(vec![
            Span::styled(" Filter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(enabled_str, param_style(0, enabled_color)),
            Span::styled("  Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled(filter_type.name(), param_style(1, Color::Magenta)),
            Span::styled("  Cutoff: ", Style::default().fg(Color::DarkGray)),
            Span::styled(cutoff_str, param_style(2, Color::White)),
            Span::styled("  Reso: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}", filter_resonance), param_style(3, Color::White)),
            Span::styled("  Env: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", filter_env_amount * 100.0), param_style(4, Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Magenta };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Filter [EDIT] " } else { " Filter " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_envelope_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    // Amplitude ADSR
    let amp_a = app.amp_attack();
    let amp_d = app.amp_decay();
    let amp_s = app.amp_sustain();
    let amp_r = app.amp_release();

    // Filter ADSR
    let flt_a = app.filter_attack();
    let flt_d = app.filter_decay();
    let flt_s = app.filter_sustain();
    let flt_r = app.filter_release();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    // Format time values nicely
    let fmt_time = |t: f32| -> String {
        if t >= 1.0 {
            format!("{:.2}s", t)
        } else {
            format!("{:.0}ms", t * 1000.0)
        }
    };

    // Params: 0=AmpA, 1=AmpD, 2=AmpS, 3=AmpR, 4=FltA, 5=FltD, 6=FltS, 7=FltR
    let content = vec![
        Line::from(vec![
            Span::styled(" Amp ADSR: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("A:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(amp_a), param_style(0, Color::White)),
            Span::styled(" D:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(amp_d), param_style(1, Color::White)),
            Span::styled(" S:", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", amp_s * 100.0), param_style(2, Color::White)),
            Span::styled(" R:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(amp_r), param_style(3, Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Flt ADSR: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled("A:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(flt_a), param_style(4, Color::White)),
            Span::styled(" D:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(flt_d), param_style(5, Color::White)),
            Span::styled(" S:", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", flt_s * 100.0), param_style(6, Color::White)),
            Span::styled(" R:", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_time(flt_r), param_style(7, Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Cyan };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Envelope [EDIT] " } else { " Envelope " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_effects_basic_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    let dist_enabled = app.distortion_enabled();
    let dist_type = app.distortion_type_name();
    let delay_enabled = app.delay_enabled();
    let delay_time = app.delay_time_ms();
    let reverb_enabled = app.reverb_enabled();
    let reverb_mix = app.reverb_mix();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let dist_color = if dist_enabled { Color::Red } else { Color::DarkGray };
    let dist_str = if dist_enabled { format!("On ({})", dist_type) } else { "Off".to_string() };

    let delay_color = if delay_enabled { Color::Blue } else { Color::DarkGray };
    let delay_str = if delay_enabled { format!("On ({:.0}ms)", delay_time) } else { "Off".to_string() };

    let reverb_color = if reverb_enabled { Color::Green } else { Color::DarkGray };
    let reverb_str = if reverb_enabled { format!("On ({:.0}%)", reverb_mix * 100.0) } else { "Off".to_string() };

    // Params: 0=Distortion, 1=Delay, 2=Reverb
    let content = vec![
        Line::from(vec![
            Span::styled(" Distortion: ", Style::default().fg(Color::DarkGray)),
            Span::styled(dist_str, param_style(0, dist_color)),
            Span::styled("  Delay: ", Style::default().fg(Color::DarkGray)),
            Span::styled(delay_str, param_style(1, delay_color)),
            Span::styled("  Reverb: ", Style::default().fg(Color::DarkGray)),
            Span::styled(reverb_str, param_style(2, reverb_color)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " toggle/adjust " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Cyan } else { Color::Yellow };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Effects [EDIT] " } else { " Effects " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_lfo_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    use crate::audio::LFODestination;

    let enabled = app.lfo_enabled();
    let waveform = app.lfo_waveform();
    let rate = app.lfo_rate();
    let depth = app.lfo_depth();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    // Build destination checkboxes
    let dest_pitch = if app.lfo_has_destination(LFODestination::Pitch) { "[x]" } else { "[ ]" };
    let dest_filter = if app.lfo_has_destination(LFODestination::FilterCutoff) { "[x]" } else { "[ ]" };
    let dest_volume = if app.lfo_has_destination(LFODestination::Volume) { "[x]" } else { "[ ]" };
    let dest_pwm = if app.lfo_has_destination(LFODestination::PulseWidth) { "[x]" } else { "[ ]" };
    let dest_osc2 = if app.lfo_has_destination(LFODestination::Osc2Pitch) { "[x]" } else { "[ ]" };
    let dest_pan = if app.lfo_has_destination(LFODestination::Pan) { "[x]" } else { "[ ]" };

    let enabled_color = if enabled { Color::Green } else { Color::Red };
    let enabled_str = if enabled { "ON" } else { "OFF" };

    // Params: 0=On/Off, 1=Wave, 2=Rate, 3=Depth
    let content = vec![
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(enabled_str, param_style(0, enabled_color)),
            Span::styled("  Wave: ", Style::default().fg(Color::DarkGray)),
            Span::styled(waveform.name(), param_style(1, Color::Cyan)),
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1} Hz", rate), param_style(2, Color::White)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", depth * 100.0), param_style(3, Color::White)),
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
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "P/F/V/W/O/A" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { " route" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Cyan };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " LFO [EDIT] " } else { " LFO " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_modulation_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    // Ring Mod
    let ring_enabled = app.ring_mod_enabled();
    let ring_freq = app.ring_mod_freq();
    let ring_mix = app.ring_mod_mix();

    // FM Synthesis
    let fm_enabled = app.fm_enabled();
    let fm_amount = app.fm_amount();
    let fm_ratio = app.fm_ratio();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let ring_color = if ring_enabled { Color::Green } else { Color::Red };
    let ring_str = if ring_enabled { "On" } else { "Off" };

    let fm_color = if fm_enabled { Color::Green } else { Color::Red };
    let fm_str = if fm_enabled { "On" } else { "Off" };

    // Params: 0=Ring on/off, 1=Ring Freq, 2=FM on/off, 3=FM Ratio
    let content = vec![
        Line::from(vec![
            Span::styled(" Ring Mod: ", Style::default().fg(Color::DarkGray)),
            Span::styled(ring_str, param_style(0, ring_color)),
            Span::styled("  Freq: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0} Hz", ring_freq), param_style(1, Color::Cyan)),
            Span::styled("  Mix: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", ring_mix * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" FM:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(fm_str, param_style(2, fm_color)),
            Span::styled("  Amount: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.2}", fm_amount), Style::default().fg(Color::Cyan)),
            Span::styled("  Ratio: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.2}", fm_ratio), param_style(3, Color::White)),
            Span::styled("  (OSC2→OSC1)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Magenta };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Modulation [EDIT] " } else { " Modulation " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_effects_ext_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    // Chorus
    let chorus_enabled = app.chorus_enabled();
    let chorus_rate = app.chorus_rate();
    let chorus_depth = app.chorus_depth();
    let chorus_voices = app.chorus_voices();

    // Phaser
    let phaser_enabled = app.phaser_enabled();
    let phaser_rate = app.phaser_rate();
    let phaser_depth = app.phaser_depth();
    let phaser_stages = app.phaser_stages();

    // Bitcrusher
    let crush_enabled = app.bitcrusher_enabled();
    let crush_bits = app.bitcrusher_bits();
    let crush_rate_div = app.bitcrusher_rate_div();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let chorus_color = if chorus_enabled { Color::Green } else { Color::Red };
    let chorus_str = if chorus_enabled { "On" } else { "Off" };

    let phaser_color = if phaser_enabled { Color::Green } else { Color::Red };
    let phaser_str = if phaser_enabled { "On" } else { "Off" };

    let crush_color = if crush_enabled { Color::Green } else { Color::Red };
    let crush_str = if crush_enabled { "On" } else { "Off" };

    // Params: 0=Chorus, 1=Phaser, 2=Bitcrusher
    let content = vec![
        Line::from(vec![
            Span::styled(" Chorus:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(chorus_str, param_style(0, chorus_color)),
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}Hz", chorus_rate), Style::default().fg(Color::Cyan)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}ms", chorus_depth), Style::default().fg(Color::White)),
            Span::styled("  Voices: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", chorus_voices), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Phaser:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(phaser_str, param_style(1, phaser_color)),
            Span::styled("  Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}Hz", phaser_rate), Style::default().fg(Color::Cyan)),
            Span::styled("  Depth: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", phaser_depth * 100.0), Style::default().fg(Color::White)),
            Span::styled("  Stages: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", phaser_stages), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Bitcrusher:", Style::default().fg(Color::DarkGray)),
            Span::styled(crush_str, param_style(2, crush_color)),
            Span::styled("  Bits: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", crush_bits), Style::default().fg(Color::Cyan)),
            Span::styled("  SR Div: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}x", crush_rate_div), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " toggle " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Cyan } else { Color::Yellow };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Effects+ [EDIT] " } else { " Effects (Extended) " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}

fn draw_performance_modal(frame: &mut Frame, area: Rect, app: &App, edit_mode: bool, param_idx: usize) {
    use crate::audio::PortamentoMode;

    // Portamento
    let portamento_mode = app.portamento_mode();
    let portamento_time = app.portamento_time();
    let porta_enabled = portamento_mode != PortamentoMode::Off;

    // Noise
    let noise_enabled = app.noise_enabled();
    let noise_type = app.noise_type();
    let noise_level = app.noise_level();

    // Arpeggiator
    let arp_enabled = app.arpeggiator_enabled();
    let arp_bpm = app.arpeggiator_bpm();
    let arp_pattern = app.arpeggiator_pattern();
    let arp_division = app.arpeggiator_division();
    let arp_octaves = app.arpeggiator_octaves();
    let arp_gate = app.arpeggiator_gate();

    // Helper for parameter highlighting
    let param_style = |idx: usize, base_color: Color| -> Style {
        if edit_mode && param_idx == idx {
            Style::default().fg(Color::Black).bg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
        }
    };

    let porta_color = if porta_enabled { Color::Green } else { Color::Red };
    let porta_str = if porta_enabled { portamento_mode.name() } else { "Off" };

    let noise_color = if noise_enabled { Color::Green } else { Color::Red };
    let noise_str = if noise_enabled { "On" } else { "Off" };

    let arp_color = if arp_enabled { Color::Green } else { Color::Red };
    let arp_str = if arp_enabled { "On" } else { "Off" };

    // Params: 0=Porta, 1=Noise, 2=NoiseType, 3=Arp, 4=Pattern, 5=Octaves
    let content = vec![
        Line::from(vec![
            Span::styled(" Portamento: ", Style::default().fg(Color::DarkGray)),
            Span::styled(porta_str, param_style(0, porta_color)),
            Span::styled("  Time: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}ms", portamento_time), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Noise:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(noise_str, param_style(1, noise_color)),
            Span::styled("  Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled(noise_type.name(), param_style(2, Color::Cyan)),
            Span::styled("  Level: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", noise_level * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Arpeggiator:", Style::default().fg(Color::DarkGray)),
            Span::styled(arp_str, param_style(3, arp_color)),
            Span::styled("  BPM: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", arp_bpm), Style::default().fg(Color::Cyan)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(arp_division.name(), Style::default().fg(Color::White)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(arp_pattern.name(), param_style(4, Color::Magenta)),
            Span::styled("  Oct: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", arp_octaves), param_style(5, Color::White)),
            Span::styled("  Gate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", arp_gate * 100.0), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(if edit_mode { " </>  " } else { " Enter " }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { "select " } else { "edit   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "^/v" } else { "Tab" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " modify " } else { " next   " }, Style::default().fg(Color::DarkGray)),
            Span::styled(if edit_mode { "Esc" } else { "" }, Style::default().fg(Color::Yellow)),
            Span::styled(if edit_mode { " back" } else { "" }, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let border_color = if edit_mode { Color::Yellow } else { Color::Green };
    let modal_widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if edit_mode { " Performance [EDIT] " } else { " Performance " })
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(modal_widget, area);
}
