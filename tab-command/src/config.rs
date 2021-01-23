use serde::Deserialize;
use std::{fs::File, io::BufReader};

/// Parses and returns the global config file contents, or returns the default config
pub fn load_global_config() -> anyhow::Result<Config> {
    let config = tab_api::config::global_config_file();
    if let None = config {
        return Ok(Config::default());
    }

    let file = File::open(config.unwrap())?;
    let reader = BufReader::new(file);
    let config: Config = serde_yaml::from_reader(reader)?;
    Ok(config)
}

/// Options that can be set in the global config file
/// Tabs are parsed as
#[derive(Deserialize, Default)]
pub struct Config {
    pub key_bindings: Option<Vec<KeyBinding>>,
    pub fuzzy: FuzzyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyConfig {
    #[serde(default = "default_create_tab")]
    pub create_tab: bool,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            create_tab: default_create_tab(),
        }
    }
}

fn default_create_tab() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyBinding {
    pub action: Action,
    pub keys: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub enum Action {
    Disconnect,
    SelectInteractive,
}
