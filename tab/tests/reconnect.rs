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
        .tab("reconnect/")
        .await_stdout("$", 5000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 1000)
        .await_stdout("foo", 1000)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("before", result.stdout);

    let result = session
        .command()
        .tab("reconnect/")
        .await_stdout("foo", 5000)
        .stdin("exit\n")
        .run()
        .await?;
    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", result.stdout);

    Ok(())
}
