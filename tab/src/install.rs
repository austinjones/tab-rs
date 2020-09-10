use std::{
    fs::File,
    io::BufWriter,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Values;

enum Config {
    Edit(PathBuf),
    Create(PathBuf),
}

impl Config {
    pub fn update<F>(&self, edit: F) -> anyhow::Result<()>
    where
        F: FnOnce(&str) -> anyhow::Result<String>,
    {
        let edited = match self {
            Self::Edit(path) => {
                println!("Editing {}...", path.to_string_lossy());
                let data = std::fs::read_to_string(path.as_path())?;
                edit(data.as_str())?
            }
            Self::Create(path) => {
                println!("Creating {}...", path.to_string_lossy());
                edit("")?
            }
        };

        self.replace(edited.as_str(), false)?;

        Ok(())
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Edit(path) => path.as_path(),
            Self::Create(path) => path.as_path(),
        }
    }

    pub fn replace(&self, data: &str, log: bool) -> anyhow::Result<()> {
        let path = self.path();

        if log {
            println!("Replacing {}...", path.to_string_lossy());
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write(data.as_bytes())?;

        Ok(())
    }
}

impl From<String> for Config {
    fn from(path: String) -> Self {
        let path: PathBuf = path.into();

        Self::from(path.as_path())
    }
}

impl From<PathBuf> for Config {
    fn from(path: PathBuf) -> Self {
        if path.exists() {
            Self::Edit(path)
        } else {
            Self::Create(path)
        }
    }
}

impl From<&Path> for Config {
    fn from(path: &Path) -> Self {
        Self::from(path.to_path_buf())
    }
}

struct ScriptConfig {
    path: PathBuf,
    shell: Shell,
}

enum Shell {
    Bash,
    Zsh,
}

enum ScriptAction {
    SourceFile(PathBuf),
}

impl ScriptAction {
    pub fn to_string(&self, shell: &Shell) -> String {
        match (self, shell) {
            (ScriptAction::SourceFile(path), Shell::Bash) => {
                format!("source {}", path.to_string_lossy())
            }
            (ScriptAction::SourceFile(path), Shell::Zsh) => {
                format!("source {}", path.to_string_lossy())
            }
        }
    }
}

impl ScriptConfig {
    pub fn new(path: PathBuf, shell: Shell) -> Self {
        Self { path, shell }
    }

    pub fn write_actions(&self, actions: &[ScriptAction]) -> anyhow::Result<()> {
        let mut data = if self.path.is_file() {
            std::fs::read_to_string(self.path.as_path())?
        } else {
            "".to_string()
        };

        let mut first_action = true;
        for action in actions {
            let action_string = action.to_string(&self.shell);
            let mut action_exists = false;
            let mut output_data = "".to_string();

            for line in data.lines() {
                if line.trim() == action_string {
                    action_exists = true;
                }

                output_data += line;
                output_data += "\n";
            }

            if !action_exists {
                println!(
                    "Adding action to {}: {}",
                    self.path.to_string_lossy(),
                    action_string
                );

                if first_action && !output_data.ends_with("\n\n") {
                    output_data += "\n";
                }

                output_data += action_string.as_str();
                output_data += "\n";
                first_action = false;
            }

            data = output_data;
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(self.path.as_path())?;
        let mut writer = BufWriter::new(file);
        writer.write(data.as_bytes())?;

        Ok(())
    }
}

pub fn run<'a>(commands: Values<'a>) -> anyhow::Result<()> {
    for command in commands {
        match command {
            "all" => install_all()?,
            "bash" => bash::install_bash()?,
            "fish" => fish::install_fish()?,
            "starship" => starship::install_starship()?,
            "zsh" => zsh::install_zsh()?,
            _ => anyhow::bail!("unsupported install command: {}", command),
        }
    }
    Ok(())
}

fn install_all() -> anyhow::Result<()> {
    if which::which("bash").is_ok() {
        bash::install_bash()?;
    }

    if which::which("fish").is_ok() {
        fish::install_fish()?;
    }

    if which::which("starship").is_ok() {
        starship::install_starship()?;
    }

    if which::which("zsh").is_ok() {
        zsh::install_zsh()?;
    }

    Ok(())
}

mod bash {
    use anyhow::anyhow;
    use std::path::PathBuf;

    use super::{Config, ScriptAction, ScriptConfig, Shell};

    pub fn install_bash() -> anyhow::Result<()> {
        println!("Installing Bash integration...");

        let bashrc = bashrc()?;
        let completion = install_completion_script()?;

        let script = ScriptConfig::new(bashrc, Shell::Bash);
        script.write_actions(&[ScriptAction::SourceFile(completion)])?;

        println!("Done.");
        println!("");

        Ok(())
    }

    fn install_completion_script() -> anyhow::Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| anyhow!("failed to resolve home directory for zshrc resolution"))?;

        path.push(".tab");
        path.push("completion");
        path.push("tab.bash");

        let config: Config = path.as_path().into();
        config.replace(include_str!("completions/bash/tab.bash"), true)?;

        Ok(path)
    }

    fn bashrc() -> anyhow::Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| anyhow!("failed to resolve home directory for zshrc resolution"))?;

        path.push(".bashrc");

        Ok(path)
    }
}

mod fish {
    use std::path::PathBuf;

    use super::Config;
    use anyhow::anyhow;

    pub fn install_fish() -> anyhow::Result<()> {
        println!("Installing Fish integration...");

        let tab_fish = include_str!("completions/fish/tab.fish");
        let path = path()?;
        let config: Config = path.into();
        config.replace(tab_fish, true)?;

        println!("Done.");
        println!("");

        Ok(())
    }

    fn path() -> anyhow::Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| anyhow!("failed to resolve home directory for fish installation"))?;

        path.push(".config");
        path.push("fish");
        path.push("completions");
        path.push("tab.fish");

        Ok(path)
    }
}

mod starship {
    use toml_edit::{table, value, Document};

    use super::Config;

    pub fn install_starship() -> anyhow::Result<()> {
        println!("Installing Starship integration...");

        let config = config_file()?;
        config.update(edit)?;

        println!("Done.");
        println!("");

        Ok(())
    }

    fn config_file() -> anyhow::Result<Config> {
        if let Ok(path) = std::env::var("STARSHIP_CONFIG") {
            return Ok(path.into());
        }

        let path = dirs::home_dir();
        if let Some(mut path) = path {
            path.push(".config");
            path.push("starship.toml");

            return Ok(path.into());
        }

        anyhow::bail!("could not resolve home directory for starship install")
    }

    fn edit(string: &str) -> anyhow::Result<String> {
        let mut toml = string.parse::<Document>()?;

        if toml["custom"].is_none() {
            toml["custom"] = table();
        }

        if toml["custom"]["tab"].is_none() {
            toml["custom"]["tab"] = table();
        }

        toml["custom"]["tab"] = table();
        toml["custom"]["tab"]["command"] = value("tab --starship");
        toml["custom"]["tab"]["when"] = value("tab --starship");
        toml["custom"]["tab"]["style"] = value("bold blue");
        toml["custom"]["tab"]["prefix"] = value("in ");
        toml["custom"]["tab"].as_inline_table();

        Ok(toml.to_string_in_original_order())
    }
}

mod zsh {
    use std::{path::PathBuf, process::Command};

    use anyhow::anyhow;
    use anyhow::bail;

    use super::{Config, ScriptAction, ScriptConfig, Shell};

    pub fn install_zsh() -> anyhow::Result<()> {
        println!("Installing Zsh integration...");

        install_completions()?;
        install_history()?;
        run_compinit()?;

        println!("Done.");
        println!("");

        Ok(())
    }

    fn run_compinit() -> anyhow::Result<()> {
        println!("Running compinit...");

        Command::new("zsh")
            .args(&["-i", "-c", "rm ~/.zcompdump*; compinit"])
            .spawn()?
            .wait()?;

        Ok(())
    }

    fn install_completions() -> anyhow::Result<()> {
        let tab_completions = include_str!("completions/zsh/_tab");
        if let Some(path) = completion_ohmyzsh() {
            let config: Config = path.into();
            config.replace(tab_completions, true)?;
            Ok(())
        } else if let Some(path) = completion_usr_local_share() {
            let config: Config = path.into();
            config.replace(tab_completions, true)?;
            Ok(())
        } else {
            bail!("failed to resolve a writable completion location: supported paths are '~/.oh-my-zsh/completions' and '/usr/local/share/zsh/site-functions'")
        }
    }

    fn install_history() -> anyhow::Result<()> {
        let zshrc = zshrc()?;
        let history_script = install_history_script()?;
        let script = ScriptConfig::new(zshrc, Shell::Zsh);

        script.write_actions(&[ScriptAction::SourceFile(history_script)])?;

        Ok(())
    }

    fn install_history_script() -> anyhow::Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| anyhow!("failed to resolve home directory for zshrc resolution"))?;

        path.push(".tab");
        path.push("completion");
        path.push("zsh-history.zsh");

        let config: Config = path.as_path().into();
        config.replace(include_str!("completions/zsh/history.zsh"), true)?;

        Ok(path)
    }

    fn zshrc() -> anyhow::Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| anyhow!("failed to resolve home directory for zshrc resolution"))?;

        path.push(".zshrc");

        Ok(path)
    }

    fn completion_ohmyzsh() -> Option<PathBuf> {
        let path = dirs::home_dir();
        if let Some(mut path) = path {
            path.push(".oh-my-zsh");
            if !path.exists() {
                return None;
            }

            path.push("completions");
            path.push("_tab");

            return Some(path);
        }

        return None;
    }

    fn completion_usr_local_share() -> Option<PathBuf> {
        let mut path = PathBuf::from("/");
        path.push("usr");
        path.push("local");
        path.push("share");
        path.push("zsh");
        path.push("site-functions");

        if !path.exists() {
            return None;
        }

        path.push("_tab");
        return Some(path);
    }
}
