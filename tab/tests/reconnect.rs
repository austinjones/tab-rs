use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that a session can be established, disconnected from, and re-established
/// Covers connection, ctrl-W, disconnection, and scrollback
#[tokio::test]
async fn reconnect() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    for i in 0..5 {
        reconnect_iter(&mut session, i).await?;
    }

    Ok(())
}

async fn reconnect_iter(session: &mut TestSession, iter: usize) -> anyhow::Result<()> {
    let tab = format!("reconnect/{}/", iter);
    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 5000)
        .stdin("echo foo\n")
        .await_stdout("echo foo", 1000)
        .await_stdout("$", 1000)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("before", result.stdout);

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("foo", 5000)
        .stdin("exit\n")
        .await_stdout("exit", 200)
        .run()
        .await?;
    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", result.stdout);

    Ok(())
}
