use std::io::stdout;
use std::panic::{set_hook, take_hook};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::flag;

/// Restore the terminal to its original state.
///
/// Disables raw mode, leaves alternate screen, and shows cursor.
/// Uses `let _ =` to suppress errors because cleanup must never panic --
/// this function is called from panic hooks and signal handlers.
pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

/// Install a panic hook that restores the terminal before delegating
/// to the original hook.
///
/// Must be called BEFORE `ratatui::init()` so the hook is in place
/// when the terminal enters alternate screen / raw mode.
pub fn install_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));
}

/// Register signal handlers for SIGINT, SIGTERM, and SIGHUP.
///
/// Returns a shared `AtomicBool` that is set to `true` when any of
/// these signals is received. The main event loop should check this
/// flag on every iteration and exit gracefully when it becomes true.
pub fn install_signal_handlers() -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGINT, Arc::clone(&shutdown)).expect("register SIGINT handler");
    flag::register(SIGTERM, Arc::clone(&shutdown)).expect("register SIGTERM handler");
    flag::register(SIGHUP, Arc::clone(&shutdown)).expect("register SIGHUP handler");
    shutdown
}
