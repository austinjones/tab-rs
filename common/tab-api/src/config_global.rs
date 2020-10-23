/// The full path to the global configuration file
pub fn global_config_file() -> Option<PathBuf> {
    if let Some(mut home_path) = dirs::home_dir() {
        home_path.push(".config");
        home_path.push("tab-config.yml");

        if home_path.exists() {
            return Some(home_path);
        }
    }

    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("tab-config.yml");

        if config_path.exists() {
            return Some(config_path);
        }
    }

    None
}

pub fn global_config() -> anyhow::Result<GlobalConfig> {}

#[derive(Deserialize)]
pub struct GlobalConfig {
    key_bindings: GlobalKeybindings,
}

pub struct GlobalKeybindings {
    #[serde(default = "default_keybinding_tab")]
    tab: String,
    disconnect: Option<String>,
    close: Option<String>,
}

fn default_keybinding_tab() -> String {
    // ctrl-T
    "\x14"
}
