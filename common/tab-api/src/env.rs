use tokio::process::Command;

/// Instructs the command module that it should interact with the terminal in raw mode
/// If false, the environment may not be a terminal pty.
pub fn is_raw_mode() -> bool {
    std::env::var("TAB_RAW_MODE")
        .ok()
        .map(|raw| raw.parse().unwrap_or(true))
        .unwrap_or(true)
}

/// Environment variables that should be forwarded from the command, to Daemon and pty processes.
pub const FORWARD_ENV_VARS: &[&str] = &["TAB_RUNTIME_DIR", "TAB_RAW_MODE"];

pub fn forward_env(child: &mut Command) {
    for var in crate::env::FORWARD_ENV_VARS.iter().copied() {
        if let Ok(dir) = std::env::var(var) {
            child.env(var, dir);
        }
    }
}
