use anyhow::Result;
use lifeline::impl_storage_clone;
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};
use sysinfo::SystemExt;

/// Config created for each daemon process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub pid: i32,
    pub port: u16,
    pub auth_token: String,
}

impl_storage_clone!(DaemonConfig);

/// The full path to tab's dotdir directory, that can be used to store state for the user.
pub fn dotdir_path() -> Result<PathBuf> {
    let mut dir = dirs::home_dir().ok_or_else(|| anyhow::Error::msg("home_dir not found"))?;

    dir.push(".tab");

    Ok(dir)
}

/// The full path to the daemon's pidfile, used to identify the running process, and the available websocket port.
/// Also stores an auth token that is required (in the Authorization header) to connect to the daemon.
pub fn daemon_file() -> Result<PathBuf> {
    let mut dir = dotdir_path()?;
    dir.push("daemon-pid.yml");
    Ok(dir)
}

/// Determines if there is an active daemon, by checking the pidfile and the active system processes.
pub fn is_running(config: &DaemonConfig) -> bool {
    let mut system = sysinfo::System::new_all();
    system.refresh_process(config.pid);

    let running = system.get_processes();
    if running.contains_key(&config.pid) {
        true
    } else {
        false
    }
}

/// Returns the path to the daemon's logfile.
pub fn daemon_log() -> Result<PathBuf> {
    let mut dir = dotdir_path()?;
    dir.push("daemon.log");
    Ok(dir)
}

/// Returns the path to a unique logfile fro the given shell process, and tab name.
pub fn history_path(shell: &str, name: &str) -> Result<PathBuf> {
    let mut path = dotdir_path()?;
    path.push("history");

    let filename = format!("history-{}-{}.txt", shell, name);
    path.push(filename);

    Ok(path)
}

/// Loads & deserializes the `DaemonConfig` from the daemon pidfile.
pub fn load_daemon_file() -> anyhow::Result<Option<DaemonConfig>> {
    let path = daemon_file()?;

    if !path.is_file() {
        log::trace!("File {:?} does not exist", path.as_path());
        return Ok(None);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config = serde_yaml::from_reader(reader)?;

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::{daemon_file, dotdir_path};

    #[test]
    fn dotdir_path_matches() {
        let mut expected = dirs::home_dir().expect("home dir required");
        expected.push(".tab");

        let path = dotdir_path();
        assert!(path.is_ok());
        assert_eq!(expected, path.unwrap());
    }

    #[test]
    fn daemonfile_path_matches() {
        let mut expected = dirs::home_dir().expect("home dir required");
        expected.push(".tab");
        expected.push("daemon-pid.yml");

        let path = daemon_file();
        assert!(path.is_ok());
        assert_eq!(expected, path.unwrap());
    }
}
