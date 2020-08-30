use anyhow::Context;
use insta::assert_debug_snapshot;
use lifeline::assert_completes;
use std::process::Stdio;
use tempfile::tempdir;

use tokio::{io::AsyncReadExt, io::AsyncWriteExt, time};

/// Time to wait for the daemon to launch
const INIT_DELAY_MS: u64 = 1200;

#[tokio::test]
async fn test_session() -> anyhow::Result<()> {
    let dir = tempdir().context("failed to create tempdir")?;
    println!("launching tests in dir: {}", dir.path().to_string_lossy());
    let command = assert_cmd::cargo::cargo_bin("tab");

    let mut run = tokio::process::Command::new(command);
    run.arg("test/session/")
        .env("SHELL", "sh")
        .env("TAB_RUNTIME_DIR", dir.path().to_string_lossy().to_string())
        .env("TAB_RAW_MODE", "false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut child = run.spawn()?;
    let mut stdin = child.stdin.take().expect("couldn't get child stdin");

    stdin.write_all("echo foo\n".as_bytes()).await?;
    stdin.write_all("exit\n".as_bytes()).await?;
    stdin.flush().await?;

    assert_completes!(
        async move {
            let mut output = child.stdout.take().expect("couldn't get child stdout");
            let mut output_string = "".to_string();
            output
                .read_to_string(&mut output_string)
                .await
                .expect("couldn't read to string");
            assert_debug_snapshot!("result", output_string);
        },
        2000
    );

    Ok(())
}
