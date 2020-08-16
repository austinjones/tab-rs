use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

/// User-facing config for persistent cli & daemon behavior
#[derive(Serialize, Deserialize)]
pub struct Config {}

impl Default for Config {
    fn default() -> Self {
        Config {}
    }
}

/// Config created for each daemon process
#[derive(Serialize, Deserialize)]
pub struct DaemonConfig {
    pub pid: u32,
    pub port: u16,
}

pub fn dotdir_path() -> Result<PathBuf> {
    let mut dir = dirs::home_dir().ok_or_else(|| anyhow::Error::msg("home_dir not found"))?;

    dir.push(".tab");

    Ok(dir)
}

pub fn daemon_file() -> Result<PathBuf> {
    let mut dir = dotdir_path()?;
    dir.push("daemon-pid.yml");
    Ok(dir)
}

pub fn daemon_log() -> Result<PathBuf> {
    let mut dir = dotdir_path()?;
    dir.push("daemon.log");
    Ok(dir)
}

pub fn config_path() -> Result<PathBuf> {
    let mut path = dotdir_path()?;
    path.push("tab.yml");
    Ok(path)
}

pub fn load_config() -> anyhow::Result<Config> {
    let path = config_path()?;

    if !path.is_file() {
        let config = Config::default();
        write_config(path.as_path(), &config)?;

        return Ok(config);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config = serde_yaml::from_reader(reader)?;

    Ok(config)
}

pub fn load_daemon_file() -> anyhow::Result<Option<DaemonConfig>> {
    let path = daemon_file()?;

    if !path.is_file() {
        log::debug!("File {:?} does not exist", path.as_path());
        return Ok(None);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config = serde_yaml::from_reader(reader)?;

    Ok(Some(config))
}

pub fn write_config(path: &Path, config: &Config) -> Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{config_path, daemon_file, dotdir_path};

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

    #[test]
    fn config_path_matches() {
        let mut expected = dirs::home_dir().expect("home dir required");
        expected.push(".tab");
        expected.push("tab.yml");

        let path = config_path();
        assert!(path.is_ok());
        assert_eq!(expected, path.unwrap());
    }
}
