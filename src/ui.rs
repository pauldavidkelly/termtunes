use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, LineGauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppView};

/// Minimum terminal width below which we show a "too small" message.
const MIN_WIDTH: u16 = 20;

/// Minimum terminal height below which we show a "too small" message.
const MIN_HEIGHT: u16 = 5;

/// Width threshold below which the UI switches to narrow/simplified layout.
const NARROW_WIDTH: u16 = 40;

/// Render the full UI frame based on the current app state.
///
/// Layout: vertical split with main content area (Fill) and either a 3-line
/// player bar (when a track is playing) or a 1-line status bar at the bottom.
///
/// Handles adaptive layout: shows a "too small" message for very small terminals,
/// and switches to a simplified narrow layout for panes under 40 columns wide.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let width = area.width;

    // Minimum viable display -- below this, just show a message
    if width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = Paragraph::new("Terminal too small")
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let is_narrow = width < NARROW_WIDTH;

    let has_player_bar = app.now_playing().is_some();
    let bar_height = if has_player_bar { 3 } else { 1 };

    let [main_area, bar_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(bar_height)]).areas(area);

    // Render main content based on current view
    match app.view() {
        AppView::Playlists => render_playlists(frame, app, main_area, width),
        AppView::Tracks | AppView::Playing => render_tracks(frame, app, main_area, width),
        AppView::Downloading => render_downloading(frame, main_area),
    }

    // Render player bar or status bar
    if has_player_bar {
        render_player_bar(frame, app, bar_area, is_narrow, width);
    } else {
        render_status_bar(frame, app, bar_area, is_narrow);
    }
}

/// Render the playlist list view.
///
/// Truncates playlist names to fit available width. In narrow mode, the track
/// count suffix is dropped first before truncating the title.
fn render_playlists(frame: &mut Frame, app: &mut App, area: Rect, width: u16) {
    let favorites = app.favorites();
    // Available chars: width minus borders (2) and highlight symbol (">> " = 2)
    let available = (width as usize).saturating_sub(4);

    let items: Vec<ListItem> = app
        .playlists()
        .iter()
        .map(|p| {
            // Check if this playlist has a favorite key assigned
            let fav_prefix = favorites
                .iter()
                .find(|(_, fav)| fav.rating_key == p.rating_key)
                .map(|(key, _)| format!("[{}] ", key))
                .unwrap_or_default();

            let count = p
                .leaf_count
                .map(|c| format!(" ({} tracks)", c))
                .unwrap_or_default();

            let full = format!("{}{}{}", fav_prefix, p.title, count);
            // If it fits, use the full string; otherwise drop count first, then truncate
            let display = if full.chars().count() <= available {
                full
            } else {
                let without_count = format!("{}{}", fav_prefix, p.title);
                if without_count.chars().count() <= available {
                    without_count
                } else {
                    truncate_for_display(&without_count, available)
                }
            };
            ListItem::new(display)
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
/// in green+bold. Other tracks show a normal "  " prefix. Track names are
/// truncated with ellipsis to fit the available width.
fn render_tracks(frame: &mut Frame, app: &mut App, area: Rect, width: u16) {
    let playing_index = app.current_track_index();
    // Available chars: width minus borders (2) and highlight symbol (">> " = 2)
    let available = (width as usize).saturating_sub(4);

    let items: Vec<ListItem> = app
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
            let is_playing = playing_index == Some(i);

            if is_playing {
                let full = format!(">> {} - {}", t.title, artist);
                let display = truncate_for_display(&full, available);
                ListItem::new(Line::from(vec![
                    Span::styled(
                        display,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
            } else {
                let full = format!("   {} - {}", t.title, artist);
                let display = truncate_for_display(&full, available);
                ListItem::new(display)
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
///         In narrow mode, only shows state icon + track name (truncated).
/// Line 2: Progress bar (LineGauge) showing elapsed/total ratio.
/// Line 3: Playback state + volume + elapsed/total time (or error if present).
///         In narrow mode, only shows state label + time (drops volume/shuffle/repeat).
fn render_player_bar(frame: &mut Frame, app: &App, area: Rect, is_narrow: bool, width: u16) {
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

    let track_info = if is_narrow {
        // Narrow mode: show only state icon + truncated track name
        let icon_len = state_icon.chars().count();
        let max_name = (width as usize).saturating_sub(icon_len);
        let name = truncate_for_display(&np.track_name, max_name);
        Line::from(vec![
            Span::styled(state_icon, icon_style),
            Span::styled(
                name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
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
        ])
    };

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
    let status_line = if app.awaiting_favorite_key() {
        // Show favorite assignment prompt when awaiting a number key
        Line::from(vec![Span::styled(
            " Press 1-9 to assign favorite, Esc to cancel ",
            Style::default().fg(Color::Yellow),
        )])
    } else if let Some(err) = app.error_message() {
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

        let time_span = Span::styled(
            format!("{} / {}", format_duration(elapsed), format_duration(total_duration)),
            Style::default().fg(Color::White),
        );

        if is_narrow {
            // Narrow mode: only state label + time (drop volume, shuffle, repeat)
            Line::from(vec![state_label, sep, time_span])
        } else {
            let volume_pct = if let Some(player) = app.player() {
                (player.volume() * 100.0).round() as u8
            } else {
                0
            };

            let volume_span = Span::styled(
                format!("Vol: {}%", volume_pct),
                Style::default().fg(Color::White),
            );

            // Build spans incrementally to support optional shuffle/repeat indicators
            let mut spans = vec![state_label, sep.clone(), volume_span, sep.clone(), time_span];

            // Shuffle indicator (magenta)
            if app.shuffle_enabled() {
                spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    "[Shuffle]",
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Repeat indicator (blue)
            let repeat_text = app.repeat_mode().indicator();
            if !repeat_text.is_empty() {
                spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    repeat_text,
                    Style::default().fg(Color::Blue),
                ));
            }

            Line::from(spans)
        }
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
/// In narrow mode, shows abbreviated help text with just the essential keys.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect, is_narrow: bool) {
    let (text, style) = if app.awaiting_favorite_key() {
        (
            " Press 1-9 to assign favorite, Esc to cancel".to_string(),
            Style::default().fg(Color::Yellow).bg(Color::DarkGray),
        )
    } else if let Some(err) = app.error_message() {
        (
            format!(" ERROR: {} ", err),
            Style::default().fg(Color::White).bg(Color::Red),
        )
    } else if is_narrow {
        (
            " q:quit j/k:nav Enter:sel Space:pause".to_string(),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        )
    } else {
        (
            " TermTunes | q:quit  j/k:nav  Enter:select  Space:pause  n/N:next/prev  +/-:vol  s:shuffle  r:repeat  h/l:seek  f:fav  1-9:play fav"
                .to_string(),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        )
    };

    let status = Paragraph::new(Span::styled(text, style)).style(style);
    frame.render_widget(status, area);
}

/// Truncate a string to fit within `max_chars` characters, appending "..." if
/// the string is too long.
///
/// Uses `.chars()` for truncation (not byte-based) to avoid panics on
/// multi-byte UTF-8 characters.
///
/// - If the string fits within `max_chars`, returns it unchanged.
/// - If `max_chars <= 3`, returns the first `max_chars` characters (no ellipsis).
/// - Otherwise, returns the first `max_chars - 3` characters followed by "...".
fn truncate_for_display(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        s.chars().take(max_chars).collect()
    } else {
        let mut truncated: String = s.chars().take(max_chars - 3).collect();
        truncated.push_str("...");
        truncated
    }
}
