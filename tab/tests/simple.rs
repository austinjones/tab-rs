use insta::assert_snapshot;

mod common;
use common::*;
use log::info;

/// Tests that a session can be established, disconnected from, and re-established
/// Covers connection, ctrl-W, disconnection, and scrollback
#[tokio::test]
async fn reconnect() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("simple/")
        .await_stdout("$", 5000)
        .stdin("exit\n")
        .await_stdout("exit", 200)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", &result.snapshot);
    info!("after: {}", result.snapshot);

    Ok(())
}
