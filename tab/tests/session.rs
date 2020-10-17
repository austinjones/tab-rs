use common::*;
use insta::assert_snapshot;

mod common;

/// Tests that a session can be established, and terminated by the shell.
/// Covers connection, stdin/stdout, and pty shutdown/propagation.
#[tokio::test]
async fn session() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    for i in 0..10 {
        session_iter(&mut session, i).await?;
    }

    Ok(())
}

async fn session_iter(session: &mut TestSession, iter: usize) -> anyhow::Result<()> {
    let tab = format!("session/{}/", iter);

    let result = session
        .command()
        .tab(tab)
        .await_stdout("$", 5000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 1000)
        .await_stdout("$", 200)
        .stdin("exit\n")
        .await_stdout("exit", 1000)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("result", result.snapshot);

    Ok(())
}
