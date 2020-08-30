use anyhow::Context;
use lifeline::assert_completes;
use std::process::{ExitStatus, Stdio};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::{tempdir, TempDir};
use tokio::process::Child;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, time};

#[derive(Clone, Debug)]
pub enum Action {
    Delay(Duration),
    Stdin(Vec<u8>),
}

pub struct TestSession {
    binary: PathBuf,
    dir: TempDir,
}

pub struct TestCommand<'s> {
    session: &'s mut TestSession,
    pub tab: String,
    pub actions: Vec<Action>,
}

impl<'s> TestCommand<'s> {
    pub fn tab<T: ToString>(&mut self, value: T) -> &mut Self {
        self.tab = value.to_string();
        self
    }

    pub fn stdin<T: ToString>(&mut self, value: T) -> &mut Self {
        let action = Action::Stdin(value.to_string().as_bytes().to_owned());
        self.actions.push(action);
        self
    }

    pub fn stdin_bytes(&mut self, value: &[u8]) -> &mut Self {
        let vec = value.into_iter().copied().collect();
        let action = Action::Stdin(vec);
        self.actions.push(action);
        self
    }

    pub fn delay(&mut self, duration: Duration) -> &mut Self {
        let action = Action::Delay(duration);
        self.actions.push(action);
        self
    }

    pub fn delay_ms(&mut self, ms: u64) -> &mut Self {
        let action = Action::Delay(Duration::from_millis(ms));
        self.actions.push(action);
        self
    }

    pub async fn run(&mut self) -> anyhow::Result<TestResult> {
        let mut run = tokio::process::Command::new(self.session.binary());
        run.arg(self.tab.as_str())
            .env("SHELL", "/bin/sh")
            .env(
                "TAB_RUNTIME_DIR",
                self.session.dir.path().to_string_lossy().to_string(),
            )
            .env("TAB_RAW_MODE", "false")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = run.spawn()?;
        let mut stdin = child.stdin.take().expect("couldn't get child stdin");

        for action in &self.actions {
            match action {
                Action::Delay(duration) => time::delay_for(duration.clone()).await,
                Action::Stdin(input) => {
                    stdin.write_all(input.as_slice()).await?;
                    stdin.flush().await?;
                }
            }
        }

        let (code, stdout) = assert_completes!(
            async move {
                let stdout = await_stdout(&mut child).await;
                let code = child.await;
                (code, stdout)
            },
            5000
        );

        let result = TestResult {
            exit_status: code?,
            stdout,
        };

        Ok(result)
    }
}

pub struct TestResult {
    pub stdout: String,
    pub exit_status: ExitStatus,
}

impl TestSession {
    pub fn new() -> anyhow::Result<Self> {
        let dir = tempdir().context("failed to create tempdir")?;
        println!("launching tests in dir: {}", dir.path().to_string_lossy());

        let binary = assert_cmd::cargo::cargo_bin("tab");

        Ok(Self { binary, dir })
    }

    pub fn binary(&self) -> &Path {
        &self.binary.as_path()
    }

    pub fn command(&mut self) -> TestCommand {
        TestCommand {
            session: self,
            tab: "tab".into(),
            actions: Vec::new(),
        }
    }
}

pub async fn await_stdout(child: &mut Child) -> String {
    let mut output = child.stdout.take().expect("couldn't get child stdout");
    let mut output_string = "".to_string();
    output
        .read_to_string(&mut output_string)
        .await
        .expect("couldn't read to string");
    let output_string =
        strip_ansi_escapes::strip(&output_string).expect("couldn't strip escape sequences");
    let output_string =
        std::str::from_utf8(output_string.as_slice()).expect("couldn't parse stdout as utf8");
    output_string.to_string()
}
