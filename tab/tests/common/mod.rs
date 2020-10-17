#![cfg(test)]

use anyhow::Context;
use lifeline::assert_completes;
use log::*;
use std::process::{ExitStatus, Stdio};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tempfile::{tempdir, TempDir};

use tokio::{io::AsyncReadExt, io::AsyncWriteExt, time};

use simplelog::{TermLogger, TerminalMode};
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
    FinishSnapshot,
}

/// Represents a tab runtime (including command, daemon, and pty sessions)
/// The tab binary is retrieved from the built cargo bin.
pub struct TestSession {
    binary: PathBuf,
    dir: TempDir,
}

/// Represents & executes a single invocation of the `tab` binary.
pub struct TestCommand<'s> {
    session: &'s mut TestSession,
    pub tab: String,
    pub actions: Vec<Action>,
}

#[allow(dead_code)]
impl<'s> TestCommand<'s> {
    /// Sets the tab name on the session
    pub fn tab<T: ToString>(&mut self, value: T) -> &mut Self {
        self.tab = value.to_string();
        self
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
        info!("Tab command initalizing: {}", self.tab.as_str());

        let mut run = tokio::process::Command::new(self.session.binary());
        run.arg("--log")
            .arg("info")
            .arg(self.tab.as_str())
            .env("SHELL", "/bin/bash")
            .env(
                "TAB_RUNTIME_DIR",
                self.session.dir.path().to_string_lossy().to_string(),
            )
            .env("TAB_RAW_MODE", "false")
            .env("TAB", "")
            .env("TAB_ID", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

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
                            time::delay_for(duration.clone()).await
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
                                        "Stdout match for {} found at index [{}..{}]",
                                        string,
                                        search_index + index,
                                        search_index + index + match_target.len()
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
                let code = child.await;
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

        Ok(Self { binary, dir })
    }

    /// The path to the tab binary which will be executed by commands.
    pub fn binary(&self) -> &Path {
        &self.binary.as_path()
    }

    /// Constructs a new command, which can be executed to launch the tab binaries.
    pub fn command(&mut self) -> TestCommand {
        TestCommand {
            session: self,
            tab: "tab".into(),
            actions: Vec::new(),
        }
    }
}
