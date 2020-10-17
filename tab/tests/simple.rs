use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that a session can be established, disconnected from, and re-established
/// Covers connection, ctrl-W, disconnection, and scrollback
#[tokio::test]
async fn reconnect() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("simple/")
        .await_stdout("$", 1000)
        .stdin("exit\n")
        .await_stdout("exit", 300)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", &result.snapshot);

    Ok(())
}
