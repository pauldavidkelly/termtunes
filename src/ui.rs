use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// Render the full UI frame based on the current app state.
///
/// Layout: vertical split with main content area (Fill) and a single-line
/// status bar at the bottom (Length 1). The main area content changes based
/// on the current AppView (Playlists, Tracks, or Downloading).
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let [main_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

    // Render main content based on current view
    match app.view() {
        crate::app::AppView::Playlists => render_playlists(frame, app, main_area),
        crate::app::AppView::Tracks | crate::app::AppView::Playing => {
            render_tracks(frame, app, main_area)
        }
        crate::app::AppView::Downloading => render_downloading(frame, main_area),
    }

    // Render status bar (always visible)
    render_status_bar(frame, app, status_area);
}

/// Render the playlist list view.
fn render_playlists(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
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
fn render_tracks(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app
        .tracks()
        .iter()
        .map(|t| {
            let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
            ListItem::new(format!("{} - {}", t.title, artist))
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
fn render_downloading(frame: &mut Frame, area: ratatui::layout::Rect) {
    let msg = Paragraph::new("Downloading track...")
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(msg, area);
}

/// Render the status bar at the bottom of the screen.
///
/// Content varies based on playback state:
/// - No track: help text with keybindings
/// - Playing: " >> {track_name} " with green background
/// - Paused:  " || {track_name} " with yellow background
fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (text, style) = if let Some(player) = app.player() {
        if let Some(track_name) = player.current_track_name() {
            if player.is_paused() {
                (
                    format!(" || {} ", track_name),
                    Style::default().fg(Color::Black).bg(Color::Yellow),
                )
            } else if player.is_playing() {
                (
                    format!(" >> {} ", track_name),
                    Style::default().fg(Color::Black).bg(Color::Green),
                )
            } else {
                // Track finished
                (
                    format!(" -- {} (finished) ", track_name),
                    Style::default().fg(Color::White).bg(Color::DarkGray),
                )
            }
        } else {
            default_status_bar()
        }
    } else {
        default_status_bar()
    };

    let status = Paragraph::new(Span::styled(text, style)).style(style);
    frame.render_widget(status, area);
}

/// Default status bar content when no track is playing.
fn default_status_bar() -> (String, Style) {
    (
        " TermTunes | q:quit j/k:navigate Enter:select".to_string(),
        Style::default().fg(Color::White).bg(Color::DarkGray),
    )
}
