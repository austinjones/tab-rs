use common::*;
use insta::assert_snapshot;

mod common;

/// Tests that a session can be established, and terminated by the shell.
/// Covers connection, stdin/stdout, and pty shutdown/propagation.
#[tokio::test]
async fn session() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    for i in 0..10 {
        session_iter(&session, i).await?;
    }

    Ok(())
}

async fn session_iter(session: &TestSession, iter: usize) -> anyhow::Result<()> {
    let tab = format!("session/{}/", iter);

    let result = session
        .command()
        .tab(tab)
        .await_stdout("$", 3000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 300)
        .await_stdout("$", 300)
        .stdin("exit\n")
        .await_stdout("exit", 300)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("result", result.snapshot);

    Ok(())
}
