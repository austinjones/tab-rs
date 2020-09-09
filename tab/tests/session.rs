use common::*;
use insta::assert_snapshot;

mod common;

/// Tests that a session can be established, and terminated by the shell.
/// Covers connection, stdin/stdout, and pty shutdown/propagation.
#[tokio::test]
async fn session() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("session/")
        .await_stdout("$", 5000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 1000)
        .await_stdout("$", 200)
        .stdin("exit\n")
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("result", result.stdout);

    Ok(())
}
