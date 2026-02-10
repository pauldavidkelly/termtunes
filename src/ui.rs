use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, LineGauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppView};

/// Render the full UI frame based on the current app state.
///
/// Layout: vertical split with main content area (Fill) and either a 3-line
/// player bar (when a track is playing) or a 1-line status bar at the bottom.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let has_player_bar = app.now_playing().is_some();
    let bar_height = if has_player_bar { 3 } else { 1 };

    let [main_area, bar_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(bar_height)]).areas(area);

    // Render main content based on current view
    match app.view() {
        AppView::Playlists => render_playlists(frame, app, main_area),
        AppView::Tracks | AppView::Playing => render_tracks(frame, app, main_area),
        AppView::Downloading => render_downloading(frame, main_area),
    }

    // Render player bar or status bar
    if has_player_bar {
        render_player_bar(frame, app, bar_area);
    } else {
        render_status_bar(frame, app, bar_area);
    }
}

/// Render the playlist list view.
fn render_playlists(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .playlists()
        .iter()
        .map(|p| {
            let count = p
                .leaf_count
                .map(|c| format!(" ({} tracks)", c))
                .unwrap_or_default();
            ListItem::new(format!("{}{}", p.title, count))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Playlists ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, app.playlist_state_mut());
}

/// Render the track list view (used for both Tracks and Playing states).
///
/// When a track is currently playing, it is prefixed with ">>" and displayed
/// in green+bold. Other tracks show a normal "  " prefix.
fn render_tracks(frame: &mut Frame, app: &mut App, area: Rect) {
    let playing_index = app.current_track_index();

    let items: Vec<ListItem> = app
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
            let is_playing = playing_index == Some(i);

            if is_playing {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(">> {} - {}", t.title, artist),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
            } else {
                ListItem::new(format!("   {} - {}", t.title, artist))
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" {} ", app.current_playlist_title()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, app.track_state_mut());
}

/// Render the "Downloading..." message while a track is being fetched.
fn render_downloading(frame: &mut Frame, area: Rect) {
    let msg = Paragraph::new("Downloading track...")
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(msg, area);
}

/// Render the 3-line player bar at the bottom of the screen.
///
/// Line 1: State icon + track name + artist + album (multi-colored).
/// Line 2: Progress bar (LineGauge) showing elapsed/total ratio.
/// Line 3: Playback state + volume + elapsed/total time (or error if present).
fn render_player_bar(frame: &mut Frame, app: &App, area: Rect) {
    // Split the 3-line area into three rows of 1 line each
    let [line1_area, line2_area, line3_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Get NowPlaying data (caller guarantees this is Some)
    let np = match app.now_playing() {
        Some(np) => np,
        None => return,
    };

    // Determine playback state
    let (is_paused, is_playing) = if let Some(player) = app.player() {
        (player.is_paused(), player.is_playing())
    } else {
        (false, false)
    };

    // --- Line 1: Track info ---
    let (state_icon, icon_style) = if is_paused {
        (" || ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if is_playing {
        (" >> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        (" -- ", Style::default().fg(Color::DarkGray))
    };

    let separator_style = Style::default().fg(Color::DarkGray);

    let track_info = Line::from(vec![
        Span::styled(state_icon, icon_style),
        Span::styled(
            &np.track_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" - ", separator_style),
        Span::styled(&np.artist, Style::default().fg(Color::Cyan)),
        Span::styled(" - ", separator_style),
        Span::styled(&np.album, Style::default().fg(Color::Yellow)),
    ]);

    frame.render_widget(Paragraph::new(track_info), line1_area);

    // --- Line 2: Progress bar ---
    let (elapsed, total_duration) = if let Some(player) = app.player() {
        let elapsed = player.get_pos();
        let total = std::time::Duration::from_millis(np.duration_ms);
        (elapsed, total)
    } else {
        (std::time::Duration::ZERO, std::time::Duration::from_millis(np.duration_ms))
    };

    // Compute ratio, clamped to 0.0..=1.0 to prevent LineGauge panic
    let ratio = if total_duration.as_secs_f64() > 0.0 {
        (elapsed.as_secs_f64() / total_duration.as_secs_f64()).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = LineGauge::default()
        .ratio(ratio)
        .filled_style(Style::default().fg(Color::Cyan))
        .unfilled_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(gauge, line2_area);

    // --- Line 3: Status (or error if present) ---
    let status_line = if let Some(err) = app.error_message() {
        // Show error in red on line 3 instead of normal status
        Line::from(vec![
            Span::styled(
                format!(" ERROR: {} ", err),
                Style::default().fg(Color::White).bg(Color::Red),
            ),
        ])
    } else {
        let state_label = if is_paused {
            Span::styled(" Paused ", Style::default().fg(Color::Yellow))
        } else if is_playing {
            Span::styled(" Playing ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" Stopped ", Style::default().fg(Color::DarkGray))
        };

        let sep = Span::styled(" | ", Style::default().fg(Color::DarkGray));

        let volume_pct = if let Some(player) = app.player() {
            (player.volume() * 100.0).round() as u8
        } else {
            0
        };

        let volume_span = Span::styled(
            format!("Vol: {}%", volume_pct),
            Style::default().fg(Color::White),
        );

        let time_span = Span::styled(
            format!("{} / {}", format_duration(elapsed), format_duration(total_duration)),
            Style::default().fg(Color::White),
        );

        Line::from(vec![state_label, sep.clone(), volume_span, sep, time_span])
    };

    frame.render_widget(Paragraph::new(status_line), line3_area);
}

/// Format a Duration as "MM:SS".
fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

/// Render the single-line status bar when no track is playing.
///
/// Shows error messages (red) if present, otherwise shows keybinding help.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (text, style) = if let Some(err) = app.error_message() {
        (
            format!(" ERROR: {} ", err),
            Style::default().fg(Color::White).bg(Color::Red),
        )
    } else {
        (
            " TermTunes | q:quit  j/k:navigate  Enter:select  Space:pause  n/N:next/prev  +/-:volume"
                .to_string(),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        )
    };

    let status = Paragraph::new(Span::styled(text, style)).style(style);
    frame.render_widget(status, area);
}
