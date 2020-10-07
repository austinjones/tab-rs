use anyhow::Result;
use lifeline::impl_storage_clone;
use serde::Deserialize;
use serde::Serialize;
use std::{env, fs::File, io::BufReader, path::PathBuf};
use sysinfo::{RefreshKind, SystemExt};

/// Config created for each daemon process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub pid: i32,
    pub port: u16,
    pub auth_token: String,
}

impl_storage_clone!(DaemonConfig);

/// Creates the data path.
pub fn mkdir() -> Result<()> {
    let data_path = data_path()?;
    std::fs::create_dir_all(data_path)?;
    Ok(())
}

/// The full path to tab's data directory, which can be used to store state for the user.
pub fn data_path() -> Result<PathBuf> {
    if let Ok(var) = env::var("TAB_RUNTIME_DIR") {
        return Ok(PathBuf::from(var));
    }

    let mut dir = dirs::data_dir().ok_or_else(|| anyhow::Error::msg("tab data dir not found"))?;

    dir.push("tab");

    Ok(dir)
}

/// The full path to the daemon's pidfile, used to identify the running process, and the available websocket port.
/// Also stores an auth token that is required (in the Authorization header) to connect to the daemon.
pub fn daemon_file() -> Result<PathBuf> {
    let mut dir = data_path()?;
    dir.push("daemon-pid.yml");
    Ok(dir)
}

/// Determines if there is an active daemon, by checking the pidfile and the active system processes.
pub fn is_running(config: &DaemonConfig) -> bool {
    let mut system = sysinfo::System::new_with_specifics(RefreshKind::new());
    system.refresh_process(config.pid);

    system.get_process(config.pid).is_some()
}

/// Returns the path to the daemon's logfile.
pub fn daemon_log() -> Result<PathBuf> {
    let mut dir = data_path()?;
    dir.push("daemon.log");
    Ok(dir)
}

/// Returns the path to a unique logfile fro the given shell process, and tab name.
pub fn history_path(shell: &str, name: &str) -> Result<PathBuf> {
    let mut path = data_path()?;
    path.push("history");

    let name = name.replace("/", "_");

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
    use super::{daemon_file, data_path};

    #[test]
    fn data_path_matches() {
        let mut expected = dirs::data_dir().expect("home dir required");
        expected.push("tab");

        let path = data_path();
        assert!(path.is_ok());
        assert_eq!(expected, path.unwrap());
    }

    #[test]
    fn daemonfile_path_matches() {
        let mut expected = dirs::data_dir().expect("home dir required");
        expected.push("tab");
        expected.push("daemon-pid.yml");

        let path = daemon_file();
        assert!(path.is_ok());
        assert_eq!(expected, path.unwrap());
    }
}
