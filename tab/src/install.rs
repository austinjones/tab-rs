use anyhow::{anyhow, bail, Context};
use clap::Values;
use dialoguer::Confirm;
use std::{
    fs::{File, Permissions},
    io::BufReader,
    io::BufWriter,
    io::Read,
    io::Write,
    os::unix::prelude::PermissionsExt,
    path::Path,
    path::PathBuf,
};

mod bash;
mod fish;
mod starship;
mod zsh;

pub fn run<'a>(commands: Values<'a>) -> anyhow::Result<()> {
    let env = PackageEnv::new()?;

    for command in commands {
        let mut packages = match command {
            "all" => package_all(&env)?,
            "bash" => vec![bash::bash_package(&env)],
            "fish" => vec![fish::fish_package(&env)],
            "starship" => vec![starship::starship_package(&env)],
            "zsh" => vec![zsh::zsh_package(&env)?],
            _ => anyhow::bail!("unsupported install command: {}", command),
        };

        for package in packages.iter_mut() {
            package.sort();
        }

        // print packages
        let package_len = packages.len();
        eprintln!("Found {} installable packages.", package_len);
        eprintln!("");

        for package in &packages {
            eprint!("{}", package.to_string());
        }

        if !Confirm::new()
            .with_prompt("Do you wish to apply the modifications?")
            .interact()?
        {
            eprintln!("");
            eprintln!("Aborted.");
            return Ok(());
        }

        eprintln!("");

        // apply packages
        for package in packages {
            let name = package.name.clone();

            eprintln!("Installing {}...", package.name.as_str());
            install_package(package).context(format!("{} installation failed:", name))?;
        }

        eprintln!("");

        eprintln!("Installed {} packages.", package_len)
    }
    Ok(())
}

fn package_all(env: &PackageEnv) -> anyhow::Result<Vec<Package>> {
    let mut packages = Vec::new();
    if which::which("bash").is_ok() {
        packages.push(bash::bash_package(env));
    }

    if which::which("fish").is_ok() {
        packages.push(fish::fish_package(env));
    }

    if which::which("starship").is_ok() {
        packages.push(starship::starship_package(env));
    }

    if which::which("zsh").is_ok() {
        packages.push(zsh::zsh_package(env)?);
    }

    Ok(packages)
}

fn install_package(package: Package) -> anyhow::Result<()> {
    for pre in package.pre_actions {
        (pre.apply)()?;
    }

    for clean in package.clean_files {
        if clean.is_file() {
            std::fs::remove_file(clean.as_path()).context(format!(
                "Failed to remove '{}'",
                clean.to_string_lossy().to_string()
            ))?;
        } else if clean.is_dir() {
            eprintln!("WARN: directory removal is unsupported");
        }
    }

    for write in package.write_files {
        safe_write(write.path.as_path(), write.contents, write.permissions).context(format!(
            "Failed to write '{}'",
            write.path.to_string_lossy().to_string()
        ))?;
    }

    for edit in package.edit_files {
        let contents = read_file(edit.path.as_path())?;
        let edited = (edit.apply)(contents);
        safe_write(edit.path.as_path(), edited, edit.permissions).context(format!(
            "Failed to edit '{}'",
            edit.path.to_string_lossy().to_string()
        ))?
    }

    for post in package.post_actions {
        (post.apply)()?;
    }

    Ok(())
}

fn read_file(path: &Path) -> anyhow::Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut string = "".to_string();
    reader.read_to_string(&mut string)?;

    Ok(Some(string))
}

/// Writes the contents to the target path, using a tempfile and a filesystem copy
fn safe_write(path: &Path, contents: String, new_permissions: Permissions) -> anyhow::Result<()> {
    if path.is_dir() {
        bail!(format!(
            "tab needs to write a file to {}, but it is a directory",
            path.to_string_lossy()
        ))
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut temp_path = path.to_path_buf();
    if !temp_path.set_extension("tabtmp") {
        bail!(format!(
            "failed to set file extension for: {}",
            path.to_string_lossy()
        ));
    }

    match unsafe_write_file(temp_path.as_path(), contents) {
        Ok(_) => {}
        Err(e) => {
            if temp_path.is_file() {
                if let Err(e) = std::fs::remove_file(temp_path.as_path()) {
                    eprintln!(
                        "failed to remove tempfile: {}, {}",
                        e,
                        temp_path.to_string_lossy()
                    );
                }
                return Err(e);
            }
        }
    }

    if path.is_file() {
        let permissions = File::open(path)?.metadata()?.permissions();
        std::fs::set_permissions(temp_path.as_path(), permissions)?;
    } else {
        std::fs::set_permissions(temp_path.as_path(), new_permissions)?;
    }

    std::fs::copy(temp_path.as_path(), path)?;
    std::fs::remove_file(temp_path)?;

    Ok(())
}

fn unsafe_write_file(path: &Path, contents: String) -> anyhow::Result<()> {
    let tempfile = File::create(path)?;
    let mut writer = BufWriter::new(tempfile);
    writer.write_all(contents.as_bytes())?;

    Ok(())
}

/// Environment information for package construction
pub struct PackageEnv {
    pub home: PathBuf,
    pub data: PathBuf,
}

impl PackageEnv {
    pub fn new() -> anyhow::Result<Self> {
        let home = dirs::home_dir().ok_or(anyhow!(
            "A home directory is required for package installation, and none could be found."
        ))?;

        let data = dirs::data_dir().ok_or(anyhow!(
            "A data directory is required for package installation, and none could be found."
        ))?;

        Ok(Self { home, data })
    }
}

pub struct PackageBuilder {
    pub(super) package: Package,
}

impl PackageBuilder {
    pub fn new<T: ToString>(name: T) -> Self {
        Self {
            package: Package {
                name: name.to_string(),
                pre_actions: Vec::new(),
                clean_files: Vec::new(),
                edit_files: Vec::new(),
                write_files: Vec::new(),
                post_actions: Vec::new(),
            },
        }
    }

    /// Executes an action before any file modifications have been made
    #[allow(dead_code)]
    pub fn preinstall_action<F, Desc>(&mut self, apply: F, description: Desc)
    where
        F: FnOnce() -> anyhow::Result<()> + 'static,
        Desc: ToString,
    {
        let action = PackageInstallAction {
            apply: Box::new(apply),
            description: description.to_string(),
        };

        self.package.pre_actions.push(action);
    }

    /// Removes a file, if it exists.  Useful for cleaning up files which are conditionally written.
    pub fn clean_file(&mut self, path: PathBuf) -> &mut Self {
        self.package.clean_files.push(path);
        self
    }

    /// Writes a file, creating or replacing it with the given contents.
    pub fn write_file<T: ToString, D: ToString>(
        &mut self,
        path: PathBuf,
        contents: T,
        description: D,
        permissions: Permissions,
    ) -> &mut Self {
        let write = PackageWrite {
            path,
            permissions,
            contents: contents.to_string(),
            description: description.to_string(),
        };

        self.package.write_files.push(write);
        self
    }

    /// Edits a file.  Reads the file if it exists, then uses the provided apply function, and writes it to disk.
    pub fn edit<F, Desc>(
        &mut self,
        path: PathBuf,
        permissions: Permissions,
        apply: F,
        description: Desc,
    ) -> &mut Self
    where
        F: FnOnce(Option<String>) -> String + 'static,
        Desc: ToString,
    {
        let edit = PackageEdit {
            path,
            apply: Box::new(apply),
            description: description.to_string(),
            permissions,
        };

        self.package.edit_files.push(edit);
        self
    }

    /// Edits a shell script (e.g. .bashrc or .zshrc), given a shell variant
    pub fn script<Desc>(
        &mut self,
        shell: Shell,
        path: PathBuf,
        permissions: Permissions,
        description: Desc,
    ) -> ScriptBuilder
    where
        Desc: ToString,
    {
        ScriptBuilder::new(self, shell, path, permissions, description.to_string())
    }

    /// Applies an action after all other steps have completed successfully
    pub fn postinstall_action<F, Desc>(&mut self, apply: F, description: Desc)
    where
        F: FnOnce() -> anyhow::Result<()> + 'static,
        Desc: ToString,
    {
        let action = PackageInstallAction {
            apply: Box::new(apply),
            description: description.to_string(),
        };

        self.package.post_actions.push(action);
    }

    /// Builds the package description
    pub fn build(&mut self) -> Package {
        let package = std::mem::replace(&mut self.package, Package::new(""));
        package
    }
}

pub struct ScriptBuilder<'b> {
    package_builder: &'b mut PackageBuilder,
    path: PathBuf,
    permissions: Permissions,
    script: ScriptConfig,
    description: String,
}

impl<'b> ScriptBuilder<'b> {
    /// Constructs a new shell script builder
    pub(super) fn new(
        package_builder: &'b mut PackageBuilder,
        shell: Shell,
        path: PathBuf,
        permissions: Permissions,
        description: String,
    ) -> Self {
        Self {
            package_builder,
            path,
            permissions,
            description,
            script: ScriptConfig::new(shell),
        }
    }

    pub fn action(&mut self, action: ScriptAction) -> &mut Self {
        self.script.actions.push(action);
        self
    }

    pub fn build(&'b mut self) -> &'b mut PackageBuilder {
        let script = std::mem::replace(&mut self.script, ScriptConfig::new(Shell::Bash));
        let description = std::mem::take(&mut self.description);
        let apply = move |string| script.apply(string);

        let edit = PackageEdit {
            path: self.path.clone(),
            permissions: self.permissions.clone(),
            apply: Box::new(apply),
            description,
        };

        self.package_builder.package.edit_files.push(edit);
        &mut self.package_builder
    }
}

pub struct PackageEdit {
    pub path: PathBuf,
    pub apply: Box<dyn FnOnce(Option<String>) -> String>,
    pub description: String,
    pub permissions: Permissions,
}

pub struct PackageWrite {
    pub path: PathBuf,
    pub contents: String,
    pub description: String,
    pub permissions: Permissions,
}

pub struct PackageInstallAction {
    pub description: String,
    pub apply: Box<dyn FnOnce() -> anyhow::Result<()>>,
}

/// An installable package of files and script modifications
pub struct Package {
    /// The user-facing display name for this package
    pub name: String,

    /// Pre-installation hooks
    pub pre_actions: Vec<PackageInstallAction>,

    /// All file paths known to this installer, which may have been created on a previous run
    /// Paths that appear in `files` are required.
    /// .bashrc and other dotfiles should not be provided
    pub clean_files: Vec<PathBuf>,

    /// Edits the TOML
    pub edit_files: Vec<PackageEdit>,

    /// All files which should be created, with the given contents
    pub write_files: Vec<PackageWrite>,

    /// Pre-installation hooks
    pub post_actions: Vec<PackageInstallAction>,
}

impl Package {
    pub fn new<T: ToString>(name: T) -> Self {
        Self {
            name: name.to_string(),
            pre_actions: Vec::new(),
            clean_files: Vec::new(),
            edit_files: Vec::new(),
            write_files: Vec::new(),
            post_actions: Vec::new(),
        }
    }
}

impl Package {
    pub fn sort(&mut self) {
        self.clean_files.sort();
        self.edit_files.sort_by(|a, b| a.path.cmp(&b.path));
        self.write_files.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

impl ToString for Package {
    fn to_string(&self) -> String {
        let mut string = "".to_string();

        string += format!("[Tab Package: {}]\n", self.name).as_str();

        for preinstall in &self.pre_actions {
            string += format!("{}\n", preinstall.description).as_str();
        }

        for path in self.clean_files.iter() {
            if path.is_file() {
                string += format!("Remove {}\n", path.to_string_lossy()).as_str();
            }
        }

        for write in self.write_files.iter() {
            string += format!(
                "Create {} ({})\n",
                write.path.to_string_lossy(),
                write.description.as_str()
            )
            .as_str();
        }

        for edit in self.edit_files.iter() {
            string += format!(
                "Edit {} ({})\n",
                edit.path.to_string_lossy(),
                edit.description.as_str()
            )
            .as_str();
        }

        for postinstall in &self.post_actions {
            string += format!("{}\n", postinstall.description).as_str();
        }

        string += "\n";

        string
    }
}

struct ScriptConfig {
    pub shell: Shell,
    pub actions: Vec<ScriptAction>,
}

pub enum Shell {
    Bash,
    Zsh,
}

#[allow(dead_code)]
pub enum ScriptAction {
    /// Sources the script file
    SourceFile(PathBuf),
    /// Exports an env var with the given name, and the contents
    Export(String, String),
    /// Runs a shell-specific command
    Command(String),
}

impl ScriptAction {
    pub fn to_string(&self, shell: &Shell) -> String {
        match (self, shell) {
            (ScriptAction::SourceFile(path), Shell::Bash) => {
                format!("source \"{}\"", path.to_string_lossy())
            }
            (ScriptAction::SourceFile(path), Shell::Zsh) => {
                format!("source \"{}\"", path.to_string_lossy())
            }
            (ScriptAction::Export(var, contents), Shell::Bash) => {
                format!("export {}=\"{}\"", var, contents)
            }
            (ScriptAction::Export(var, contents), Shell::Zsh) => {
                format!("export {}=\"{}\"", var, contents)
            }
            (ScriptAction::Command(command), _) => command.clone(),
        }
    }
}

enum ScanState {
    AwaitingComment,
    Cleaning,
    Complete,
}

impl ScriptConfig {
    pub fn new(shell: Shell) -> Self {
        Self {
            shell,
            actions: Vec::new(),
        }
    }

    pub fn apply(self, source: Option<String>) -> String {
        let data = source.unwrap_or("".to_string());
        let mut output_data = "".to_string();

        let mut state = ScanState::AwaitingComment;
        let action_strings: Vec<String> = self
            .actions
            .iter()
            .map(|action| action.to_string(&self.shell))
            .collect();

        'line: for line in data.lines() {
            match state {
                ScanState::AwaitingComment => {
                    if line.contains("#") && line.contains("tab multiplexer configuration") {
                        state = ScanState::Cleaning;

                        output_data += line;
                        output_data += "\n";

                        for action in &action_strings {
                            output_data += action;
                            output_data += "\n";
                        }

                        continue 'line;
                    }
                }
                ScanState::Cleaning => {
                    // while the line has visible text which isn't a comment, skip it
                    if !line.contains("#") && line.trim().len() > 0 {
                        continue 'line;
                    }

                    state = ScanState::Complete;
                }
                ScanState::Complete => {}
            }

            match state {
                ScanState::AwaitingComment | ScanState::Complete => {
                    for action in &action_strings {
                        // if we unexpectedly find one of our commands in the text, strip it from the output
                        if line.trim() == action.as_str() {
                            continue 'line;
                        }
                    }

                    if line.contains("#") && line.contains("tab multiplexer configuration") {
                        continue 'line;
                    }
                }
                _ => {}
            }

            output_data += line;
            output_data += "\n";
        }

        if let ScanState::AwaitingComment = state {
            if data.ends_with("\n\n") {
            } else if data.ends_with("\n") {
                output_data += "\n"
            } else {
                output_data += "\n\n"
            }

            output_data +=
                "# tab multiplexer configuration: https://github.com/austinjones/tab-rs/\n";

            for action in action_strings {
                output_data += action.as_str();
                output_data += "\n";
            }

            output_data += "# end tab configuration\n\n";
        }

        output_data
    }
}

impl ToString for ScriptConfig {
    fn to_string(&self) -> String {
        let mut string = "".to_string();

        for action in &self.actions {
            string += format!("+ {}\n", action.to_string(&self.shell)).as_str();
        }

        string
    }
}
