use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, LineGauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppView, BrowserState};
use crate::visualizer;

/// Minimum terminal width below which we show a "too small" message.
const MIN_WIDTH: u16 = 20;

/// Minimum terminal height below which we show a "too small" message.
const MIN_HEIGHT: u16 = 5;

/// Width threshold below which the UI switches to narrow/simplified layout.
const NARROW_WIDTH: u16 = 40;

/// Minimum terminal height to show the visualizer. Terminals shorter than
/// this auto-hide the visualizer to preserve the track list and player bar.
const MIN_VIZ_HEIGHT: u16 = 20;

/// Render the main content area based on current view.
fn render_main_content(frame: &mut Frame, app: &mut App, area: Rect, width: u16) {
    match app.view() {
        AppView::Playlists => render_playlists(frame, app, area, width),
        AppView::Tracks | AppView::Playing => render_tracks(frame, app, area, width),
        AppView::Downloading => render_downloading(frame, area),
    }
}

/// Render the full UI frame based on the current app state.
///
/// Layout: vertical split with main content area (Fill) and either a 3-line
/// player bar (when a track is playing) or a 1-line status bar at the bottom.
/// When the visualizer is enabled and space permits, a visualizer area is
/// inserted between the main content and the player bar. When an ambient
/// track is loaded, a 1-line ambient status panel is inserted above the
/// visualizer/player bar.
///
/// Four layout combinations:
/// - viz + ambient: main, ambient (1), viz (8), bar
/// - viz only:      main, viz (8), bar
/// - ambient only:  main, ambient (1), bar
/// - neither:       main, bar
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

    // Determine whether to show the visualizer: enabled, track playing,
    // not narrow terminal, and minimum height available.
    let show_viz = app.visualizer_enabled()
        && app.now_playing().is_some()
        && !is_narrow
        && area.height >= MIN_VIZ_HEIGHT;

    // Determine whether an ambient track is loaded (show ambient panel)
    let has_ambient = app.player().is_some_and(|p| p.ambient_track_name().is_some());

    if show_viz && has_ambient {
        // 4-part layout: main, ambient, viz, bar
        let [main_area, ambient_area, viz_area, bar_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(bar_height),
        ])
        .areas(area);

        render_main_content(frame, app, main_area, width);
        render_ambient_panel(frame, app, ambient_area, is_narrow);
        render_visualizer_area(frame, app, viz_area);
        render_player_bar(frame, app, bar_area, is_narrow, width);
    } else if show_viz {
        // 3-part layout: main, viz, bar
        let [main_area, viz_area, bar_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(bar_height),
        ])
        .areas(area);

        render_main_content(frame, app, main_area, width);
        render_visualizer_area(frame, app, viz_area);
        render_player_bar(frame, app, bar_area, is_narrow, width);
    } else if has_ambient {
        // 3-part layout: main, ambient, bar
        let [main_area, ambient_area, bar_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(bar_height),
        ])
        .areas(area);

        render_main_content(frame, app, main_area, width);
        render_ambient_panel(frame, app, ambient_area, is_narrow);
        if has_player_bar {
            render_player_bar(frame, app, bar_area, is_narrow, width);
        } else {
            render_status_bar(frame, app, bar_area, is_narrow);
        }
    } else {
        // 2-part layout: main, bar
        let [main_area, bar_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(bar_height)]).areas(area);

        render_main_content(frame, app, main_area, width);
        if has_player_bar {
            render_player_bar(frame, app, bar_area, is_narrow, width);
        } else {
            render_status_bar(frame, app, bar_area, is_narrow);
        }
    }

    // Render browser overlay on top of everything when open
    if !matches!(app.browser_state(), BrowserState::Closed) {
        render_browser_overlay(frame, app);
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

/// Render the audio spectrum visualizer area.
///
/// Calculates the dynamic number of bars from available width, then delegates
/// to `visualizer::render_visualizer` with the smoothed bar values from the
/// app state. Uses min(32, available_width - 2) bars.
fn render_visualizer_area(frame: &mut Frame, app: &mut App, area: Rect) {
    // Calculate dynamic bar count: available width minus borders (2),
    // clamped to 4..=64 range.
    let num_bars = ((area.width.saturating_sub(2)) as usize).clamp(4, 64);

    // Update the app's dynamic bar count for the next FFT computation
    app.set_visualizer_num_bars(num_bars);

    // Get the smoothed bar values and take only as many as we can display
    let bars = app.visualizer_bars();
    let display_bars: Vec<f64> = if bars.len() >= num_bars {
        bars[..num_bars].to_vec()
    } else {
        // If fewer bars than needed, pad with zeros
        let mut v = bars.to_vec();
        v.resize(num_bars, 0.0);
        v
    };

    visualizer::render_visualizer(frame, area, &display_bars);
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
            // Display the user's intended volume (saved_volume), NOT the
            // budget-enforced sink volume. The budget transparently scales
            // the actual sink values to prevent clipping, but the user should
            // see their intended setting (0-100%).
            let volume_pct = (app.saved_volume() * 100.0).round() as u8;

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

/// Render the 1-line ambient status panel.
///
/// Shows: state icon (AMB >> or AMB ||) + track name + separator + volume percentage.
/// Colors: Magenta for active ambient, DarkGray for muted/inactive.
/// In narrow mode: drops the volume display to save space.
fn render_ambient_panel(frame: &mut Frame, app: &App, area: Rect, is_narrow: bool) {
    let player = app.player();
    let track_name = player
        .and_then(|p| p.ambient_track_name())
        .unwrap_or("No ambient");
    let is_active = app.ambient_volume() > 0.0;

    let state_icon = if is_active {
        Span::styled(
            " AMB >> ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " AMB || ",
            Style::default().fg(Color::DarkGray),
        )
    };

    let name_span = Span::styled(
        track_name,
        Style::default().fg(if is_active {
            Color::White
        } else {
            Color::DarkGray
        }),
    );

    if is_narrow {
        let line = Line::from(vec![state_icon, name_span]);
        frame.render_widget(Paragraph::new(line), area);
    } else {
        let sep = Span::styled(" | ", Style::default().fg(Color::DarkGray));
        let vol_pct = (app.ambient_volume() * 100.0).round() as u8;
        let vol_span = Span::styled(
            format!("Vol: {}%", vol_pct),
            Style::default().fg(if is_active {
                Color::Magenta
            } else {
                Color::DarkGray
            }),
        );
        let line = Line::from(vec![state_icon, name_span, sep, vol_span]);
        frame.render_widget(Paragraph::new(line), area);
    }
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
            " q:quit j/k:nav Enter:sel Space:pause m:amb".to_string(),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        )
    } else {
        (
            " TermTunes | q:quit  j/k:nav  Enter:select  Space:pause  n/N:next/prev  +/-:vol  s:shuffle  r:repeat  h/l:seek  v:viz  f:fav  1-9:play fav  b:browse  m:amb  [/]:amb vol"
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

/// Calculate a centered popup area as a percentage of the full terminal.
///
/// Uses ratatui's Layout + Flex::Center pattern (official popup example)
/// to compute a centered Rect that automatically handles terminal resize.
fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

/// Render the ambient track browser as a centered popup overlay.
///
/// Uses the Clear widget to erase the background region, then renders
/// content based on the current BrowserState level. Highlight style uses
/// Magenta background to distinguish from main Cyan selection.
fn render_browser_overlay(frame: &mut Frame, app: &mut App) {
    let popup = popup_area(frame.area(), 70, 80);

    // Clear the popup area to prevent bleed-through from underlying content
    frame.render_widget(Clear, popup);

    let highlight_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let border_style = Style::default().fg(Color::Magenta);

    match app.browser_state_mut() {
        BrowserState::TopLevel { list_state } => {
            let items = vec![
                ListItem::new("Playlists"),
                ListItem::new("Artists"),
            ];
            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" Ambient Browser ")
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(highlight_style)
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::Artists { all_artists, filtered_indices, search_query, list_state, .. } => {
            // Split popup: top 2 lines for search bar, rest for artist list
            let [search_area, list_area] = Layout::vertical([
                Constraint::Length(3), // 1 line border top + 1 search text + 1 border bottom
                Constraint::Fill(1),
            ])
            .areas(popup);

            // Search bar
            let search_display = if search_query.is_empty() {
                Span::styled("Type to search...", Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(
                    format!("Search: {}_", search_query),
                    Style::default().fg(Color::Magenta),
                )
            };
            let title = if search_query.is_empty() {
                " Artists ".to_string()
            } else {
                format!(" Artists - {} matches ", filtered_indices.len())
            };
            let search_block = Paragraph::new(Line::from(vec![Span::raw(" "), search_display]))
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(border_style),
                );
            frame.render_widget(search_block, search_area);

            // Artist list (filtered)
            if filtered_indices.is_empty() && !search_query.is_empty() {
                let no_match = Paragraph::new("No matches")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::DarkGray))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );
                frame.render_widget(no_match, list_area);
            } else {
                let items: Vec<ListItem> = filtered_indices
                    .iter()
                    .filter_map(|&i| all_artists.get(i))
                    .map(|a| ListItem::new(a.title.clone()))
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(border_style),
                    )
                    .highlight_style(highlight_style)
                    .highlight_symbol("> ");
                frame.render_stateful_widget(list, list_area, list_state);
            }
        }
        BrowserState::Albums { artist_name, albums, list_state, .. } => {
            let items: Vec<ListItem> = albums
                .iter()
                .map(|a| {
                    let year_suffix = a.year.map(|y| format!(" ({})", y)).unwrap_or_default();
                    ListItem::new(format!("{}{}", a.title, year_suffix))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!(" {} - Albums ", artist_name))
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(highlight_style)
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::ArtistTracks { album_title, artist_name, tracks, list_state, .. } => {
            let mut items: Vec<ListItem> = Vec::with_capacity(tracks.len() + 1);
            // First item: "Play All"
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(">> Play All ({} tracks)", tracks.len()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ])));
            // Track items
            for t in tracks.iter() {
                let artist = t.artist.as_deref().unwrap_or("Unknown");
                items.push(ListItem::new(format!("  {} - {}", t.title, artist)));
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!(" {} - {} ", album_title, artist_name))
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(highlight_style)
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::Playlists { playlists, list_state } => {
            let items: Vec<ListItem> = playlists
                .iter()
                .map(|p| {
                    let count_suffix = p.leaf_count
                        .map(|c| format!(" ({} tracks)", c))
                        .unwrap_or_default();
                    ListItem::new(format!("{}{}", p.title, count_suffix))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" Playlists ")
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(highlight_style)
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::PlaylistTracks { playlist_title, tracks, list_state } => {
            let mut items: Vec<ListItem> = Vec::with_capacity(tracks.len() + 1);
            // First item: "Play All"
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(">> Play All ({} tracks)", tracks.len()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ])));
            // Track items
            for t in tracks.iter() {
                let artist = t.artist.as_deref().unwrap_or("Unknown");
                items.push(ListItem::new(format!("  {} - {}", t.title, artist)));
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!(" {} ", playlist_title))
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(highlight_style)
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::Closed => {} // Should not reach here -- caller checks
    }
}
