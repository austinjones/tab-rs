//! A global, configurable level filters
//! The command, daemon, and pty honor this level on startup.
//!
//! The main executible configures if the --log <level> option is provided
//!
//! get_level() returns None unless set_level has been called.
use std::sync::atomic::{AtomicU8, Ordering};

use log::LevelFilter;

// info level
static LOG_LEVEL: AtomicU8 = AtomicU8::new(0);

pub fn set_level(level: LevelFilter) {
    let byte_repr = discriminant_of(level);
    LOG_LEVEL.store(byte_repr, Ordering::SeqCst);
}

pub fn get_level() -> Option<LevelFilter> {
    level_of(LOG_LEVEL.load(Ordering::SeqCst))
}

pub fn get_level_str() -> Option<&'static str> {
    let level = get_level();

    if let None = level {
        return None;
    }

    match level.unwrap() {
        LevelFilter::Off => Some("off"),
        LevelFilter::Error => Some("error"),
        LevelFilter::Warn => Some("warn"),
        LevelFilter::Info => Some("info"),
        LevelFilter::Debug => Some("debug"),
        LevelFilter::Trace => Some("trace"),
    }
}

// LevelFilter has a from_usize method, but it's private
// we have to redo the cases here, and support None as 0
fn discriminant_of(filter: LevelFilter) -> u8 {
    match filter {
        LevelFilter::Trace => 1,
        LevelFilter::Debug => 2,
        LevelFilter::Info => 3,
        LevelFilter::Warn => 4,
        LevelFilter::Error => 5,
        LevelFilter::Off => 6,
    }
}

fn level_of(filter: u8) -> Option<LevelFilter> {
    match filter {
        0 => None,
        1 => Some(LevelFilter::Trace),
        2 => Some(LevelFilter::Debug),
        3 => Some(LevelFilter::Info),
        4 => Some(LevelFilter::Warn),
        5 => Some(LevelFilter::Error),
        6 => Some(LevelFilter::Off),
        _ => unreachable!("unreachable level discriminant"),
    }
}
