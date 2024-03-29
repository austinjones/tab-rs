#![cfg(test)]

use anyhow::Context;
use lifeline::assert_completes;
use log::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use std::{
    process::{ExitStatus, Stdio},
    sync::Arc,
};
use tempfile::{tempdir, TempDir};

use tokio::{io::AsyncReadExt, io::AsyncWriteExt, time};

use simplelog::{ColorChoice, TermLogger, TerminalMode};
use std::sync::Once;

static INIT: Once = Once::new();

/// Setup function that is only run once, even if called multiple times.
fn setup() {
    INIT.call_once(|| {
        TermLogger::init(
            LevelFilter::Info,
            simplelog::ConfigBuilder::new()
                .set_time_format_str("%H:%M:%S%.3f TST")
                .build(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        )
        .unwrap();
    });
}
/// An action which interacts with a running tab command
#[derive(Clone, Debug)]
pub enum Action {
    Delay(Duration),
    AwaitStdout(Vec<u8>, Duration),
    Stdin(Vec<u8>),
    Disconnect,
    FinishSnapshot,
}

/// Represents a tab runtime (including command, daemon, and pty sessions)
/// The tab binary is retrieved from the built cargo bin.
pub struct TestSession {
    _tempdir: TempDir,
    context: Arc<TestContext>,
}

struct TestContext {
    binary: PathBuf,
    runtime_dir: PathBuf,
}

/// Represents & executes a single invocation of the `tab` binary.
pub struct TestCommand {
    context: Arc<TestContext>,
    pub tab: String,
    pub actions: Vec<Action>,
    pub strict_timeout: bool,
}

#[allow(dead_code)]
impl TestCommand {
    /// Sets the tab name on the session
    pub fn tab<T: ToString>(&mut self, value: T) -> &mut Self {
        self.tab = value.to_string();
        self
    }

    /// Lists all running tab sessions
    pub async fn tabs(&self) -> anyhow::Result<Vec<String>> {
        let mut command = self.command();
        command.arg("--_autocomplete_close_tab");

        let child = command.spawn()?;
        let mut stdout = child.stdout.expect("couldn't get child stdout");

        let mut string = String::new();
        stdout.read_to_string(&mut string).await?;

        let mut tabs: Vec<String> = string
            .split("\n")
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();

        tabs.sort();

        Ok(tabs)
    }

    /// Writes stdin to the tab session
    pub fn stdin<T: ToString>(&mut self, value: T) -> &mut Self {
        let action = Action::Stdin(value.to_string().as_bytes().to_owned());
        self.actions.push(action);
        self
    }

    /// Writes raw bytes to stdin of the tab session
    pub fn stdin_bytes(&mut self, value: &[u8]) -> &mut Self {
        let vec = value.into_iter().copied().collect();
        let action = Action::Stdin(vec);
        self.actions.push(action);
        self
    }

    /// Panics if any timeout is exceeded
    pub fn strict_timeout(&mut self) -> &mut Self {
        self.strict_timeout = true;
        self
    }

    /// Writes stdin to the tab session
    pub fn await_stdout<T: ToString>(&mut self, value: T, timeout_ms: u64) -> &mut Self {
        let duration = Duration::from_millis(timeout_ms);
        let action = Action::AwaitStdout(value.to_string().as_bytes().to_owned(), duration);
        self.actions.push(action);
        self
    }

    /// Sleeps for the given duration (queued - not on the current thread)
    pub fn delay(&mut self, duration: Duration) -> &mut Self {
        let action = Action::Delay(duration);
        self.actions.push(action);
        self
    }

    /// Sleeps for the given number of milliseconds (queued - not on the current thread)
    pub fn delay_ms(&mut self, ms: u64) -> &mut Self {
        let action = Action::Delay(Duration::from_millis(ms));
        self.actions.push(action);
        self
    }

    /// Disconnects the interactive session
    pub fn disconnect(&mut self) -> &mut Self {
        let action = Action::Disconnect;
        self.actions.push(action);
        self
    }

    /// Completes the snapshot at the current stdin index
    pub fn complete_snapshot(&mut self) -> &mut Self {
        let action = Action::FinishSnapshot;
        self.actions.push(action);
        self
    }

    /// Executes all queued actions, and retrives the exit status and stdout buffer (with ansi escape codes removed).
    pub async fn run(&mut self) -> anyhow::Result<TestResult> {
        setup();

        info!("");
        info!("Tab command initializing: {}", self.tab.as_str());

        let mut run = self.command();
        run.arg(self.tab.as_str());

        let mut child = run.spawn()?;
        let mut stdin = child.stdin.take().expect("couldn't get child stdin");
        let mut stdout = child.stdout.take().expect("couldn't get child stdout");
        let mut stdout_buffer = Vec::new();
        let mut snapshot_end = None;

        assert_completes!(
            async {
                let mut search_index = 0;
                for action in &self.actions {
                    match action {
                        Action::Delay(duration) => {
                            info!("Sleeping for {:?}", &duration);
                            time::sleep(duration.clone()).await
                        }
                        Action::Stdin(input) => {
                            info!(
                                "Writing stdin: {}",
                                snailquote::escape(
                                    std::str::from_utf8(input.as_slice()).unwrap_or("")
                                )
                            );
                            stdin
                                .write_all(input.as_slice())
                                .await
                                .expect("failed to write to stdin");
                            stdin.flush().await.expect("failed to flush stdin");
                        }
                        Action::Disconnect => {
                            info!("Disconnecting session",);
                            // write `ctrl-T ESC` to stdin
                            // this is a special escape sequence which users don't use
                            stdin
                                .write_all(&[0x14, 0x03])
                                .await
                                .expect("failed to write to stdin");
                            stdin.flush().await.expect("failed to flush stdin");
                        }
                        Action::FinishSnapshot => {
                            info!("Ending snapshot at index: {}", search_index + 1);
                            snapshot_end = Some(search_index + 1);
                        }
                        Action::AwaitStdout(match_target, timeout) => {
                            let string = snailquote::escape(
                                std::str::from_utf8(match_target.as_slice()).unwrap_or(""),
                            );
                            debug!("Awaiting stdout: {}", string);
                            debug!("Current search index: {}", search_index);
                            debug!(
                                "Current stdout: {}",
                                std::str::from_utf8(stdout_buffer.as_slice()).unwrap_or("")
                            );
                            let mut buf = vec![0u8; 32];
                            let start_time = Instant::now();
                            loop {
                                debug!(
                                    "Searching from [{}..{}] in: '{}'",
                                    search_index,
                                    stdout_buffer.len(),
                                    std::str::from_utf8(&stdout_buffer[search_index..])
                                        .unwrap_or("")
                                        .replace("\r", " ")
                                        .replace("\n", " ")
                                );

                                if let Some(index) = find_subsequence(
                                    &stdout_buffer[search_index..],
                                    match_target.as_slice(),
                                ) {
                                    info!(
                                        "Stdout match for {} found at index [{}..{}] after {} ms",
                                        string,
                                        search_index + index,
                                        search_index + index + match_target.len(),
                                        Instant::now().duration_since(start_time).as_secs_f64()
                                            * 1000.
                                    );
                                    debug!(
                                        "Stdout match found at text: '{}'",
                                        std::str::from_utf8(&stdout_buffer[search_index + index..])
                                            .unwrap_or("")
                                            .replace("\r", " ")
                                            .replace("\n", " ")
                                    );
                                    search_index += index + match_target.len();
                                    break;
                                }

                                if Instant::now().duration_since(start_time) > *timeout {
                                    if self.strict_timeout {
                                        panic!("Await timeout for stdout: {}", string);
                                    }

                                    error!("Await timeout for stdout: {}", string);
                                    error!(
                                        "Current buffer: {}",
                                        snailquote::escape(
                                            std::str::from_utf8(stdout_buffer.as_slice()).unwrap()
                                        )
                                    );
                                    break;
                                }

                                let timeout = time::timeout(Duration::from_millis(1000), async {
                                    stdout
                                        .read_buf(&mut buf.as_mut_slice())
                                        .await
                                        .expect("failed to read from buf")
                                })
                                .await;

                                if let Err(_e) = timeout {
                                    warn!("Read timeout while waiting for: {}", string);
                                    continue;
                                }

                                let read = timeout.unwrap();

                                stdout_buffer.extend_from_slice(&mut buf[0..read]);
                            }
                        }
                    }
                }
            },
            30000
        );

        let code = assert_completes!(
            async {
                stdout
                    .read_to_end(&mut stdout_buffer)
                    .await
                    .expect("failed to read stdout");
                let code = child.wait().await;
                code
            },
            10000
        );

        let truncated_buffer = snapshot_end
            .map(|end| &stdout_buffer[0..end])
            .unwrap_or_else(|| stdout_buffer.as_slice());

        let stdout_buffer =
            strip_ansi_escapes::strip(&stdout_buffer).expect("couldn't strip escape sequences");

        let truncated_buffer =
            strip_ansi_escapes::strip(&truncated_buffer).expect("couldn't strip escape sequences");

        let stdout = std::str::from_utf8(stdout_buffer.as_slice())?.to_string();
        let snapshot = std::str::from_utf8(truncated_buffer.as_slice())?.to_string();

        let result = TestResult {
            exit_status: code?,
            stdout,
            snapshot,
        };

        info!("Tab command terminated: {}", self.tab.as_str());

        Ok(result)
    }

    fn command(&self) -> tokio::process::Command {
        let mut run = tokio::process::Command::new(self.context.binary.as_path());

        run.arg("--log")
            .arg("info")
            .env("SHELL", "/bin/bash")
            .env(
                "TAB_RUNTIME_DIR",
                self.context
                    .runtime_dir
                    .to_str()
                    .expect("Failed to encode runtime dir as string")
                    .to_string(),
            )
            .env("TAB_RAW_MODE", "false")
            .env("TAB", "")
            .env("TAB_ID", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        run
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The result of a `tab` command execution
/// Includes the stdout of the process (with ansi escape codes removed), and the process exit status.
pub struct TestResult {
    pub stdout: String,
    pub snapshot: String,
    pub exit_status: ExitStatus,
}

#[allow(dead_code)]
impl TestSession {
    /// Constructs a new `tab` session, generating a temp directory for the tab daemon.
    /// When the TestSession value is dropped, the daemon & pty sessions shut down.
    pub fn new() -> anyhow::Result<Self> {
        setup();

        let dir = tempdir().context("failed to create tempdir")?;
        info!("");
        info!("--------------------------------------------------------");
        info!(
            "Created test session in tempdir: {}",
            dir.path().to_string_lossy()
        );

        let binary = assert_cmd::cargo::cargo_bin("tab");

        let context = TestContext {
            runtime_dir: dir.path().to_path_buf(),
            binary,
        };

        Ok(Self {
            context: Arc::new(context),
            _tempdir: dir,
        })
    }

    /// The path to the tab binary which will be executed by commands.
    pub fn binary(&self) -> &Path {
        &self.context.binary.as_path()
    }

    /// Constructs a new command, which can be executed to launch the tab binaries.
    pub fn command(&self) -> TestCommand {
        TestCommand {
            context: self.context.clone(),
            tab: "tab".into(),
            actions: Vec::new(),
            strict_timeout: false,
        }
    }
}
