use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that a session can be established, disconnected from, and re-established
/// Covers connection, ctrl-W, disconnection, and scrollback
#[tokio::test]
async fn reconnect() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    for i in 0..10 {
        reconnect_iter(&session, i).await?;
    }

    Ok(())
}

async fn reconnect_iter(session: &TestSession, iter: usize) -> anyhow::Result<()> {
    let tab = format!("reconnect/{}/", iter);
    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 3000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 300)
        .await_stdout("$", 300)
        .disconnect()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("before", result.snapshot);

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("foo", 1000)
        .stdin("exit\n")
        .await_stdout("exit", 300)
        .complete_snapshot()
        .run()
        .await?;
    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", &result.snapshot);

    Ok(())
}
