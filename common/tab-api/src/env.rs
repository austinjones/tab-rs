use log::info;
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
pub const FORWARD_ENV_VARS: &[&str] = &[
    "TAB_RUNTIME_DIR", // The daemon & pty should inherit the runtime directory of the command client
    "TAB_RAW_MODE", // Raw mode controls stderr forwarding.  When disabled, the command stderr pipe is inherited by the daemon/client
];

pub fn forward_env(child: &mut Command) {
    for var in crate::env::FORWARD_ENV_VARS.iter().copied() {
        if let Ok(value) = std::env::var(var) {
            info!("forwarding env {} as {}", var, &value);
            child.env(var, value);
        }
    }
}

pub fn forward_env_std(child: &mut std::process::Command) {
    for var in crate::env::FORWARD_ENV_VARS.iter().copied() {
        if let Ok(value) = std::env::var(var) {
            info!("forwarding env {} as {}", var, &value);
            child.env(var, value);
        }
    }
}
