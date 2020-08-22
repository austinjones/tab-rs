use log::{debug, error, warn};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use tab_api::config::{daemon_file, is_running, load_config, load_daemon_file, DaemonConfig};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonConfigError {
    #[error("the daemon is already running")]
    AlreadyRunning,
}

pub struct DaemonFile {
    pid: i32,
    path: PathBuf,
}

impl DaemonFile {
    // TODO: doesn't work if ~/.tab doesn't exist for some reason
    pub fn new(config: &DaemonConfig) -> anyhow::Result<DaemonFile> {
        let daemon_file = daemon_file()?;

        if daemon_file.exists() {
            debug!("daemon_file already exists");
            let stored = load_daemon_file()?;
            if let Some(stored) = stored {
                debug!("retrieved stored daemon_file: {:?}", stored);

                if is_running(&stored) {
                    debug!("daemon running at pid {}, terminating.", stored.pid);
                    return Err(DaemonConfigError::AlreadyRunning.into());
                } else {
                    debug!("daemon not running at pid {}, replacing.", stored.pid);
                }
            }
        } else {
            debug!("daemonfile does not exist at {:?}", daemon_file);
        }

        std::fs::create_dir_all(daemon_file.parent().unwrap())?;
        let file = File::create(daemon_file.as_path())?;
        let buf_writer = BufWriter::new(file);
        serde_yaml::to_writer(buf_writer, config)?;

        debug!("writing daemonfile: {:?}", &config);
        let daemon_file = DaemonFile {
            pid: config.pid,
            path: daemon_file,
        };

        Ok(daemon_file)
    }

    /// Deletes the daemonfile, if the serialized pid matches this pid.
    pub fn try_drop(&mut self) -> anyhow::Result<()> {
        let config = self.reload_config()?;

        if config.pid == self.pid {
            debug!("removing pidfile: {}", self.path.display());
            std::fs::remove_file(self.path.as_path())?;
        } else {
            warn!(
                "not removing pidfile - does not contain my pid: {}",
                self.path.display()
            );
        }

        Ok(())
    }

    fn reload_config(&self) -> anyhow::Result<DaemonConfig> {
        let file = File::open(self.path.as_path())?;
        let buf_reader = BufReader::new(file);
        let config: DaemonConfig = serde_yaml::from_reader(buf_reader)?;
        Ok(config)
    }
}

impl Drop for DaemonFile {
    fn drop(&mut self) {
        let result = self.try_drop();
        if let Err(e) = result {
            error!("failed to drop pidfile: {}", e);
        }
    }
}
