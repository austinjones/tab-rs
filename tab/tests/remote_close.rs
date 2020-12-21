use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
#[tokio::test]
async fn remote_close() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    remote_close_env(&session).await?;

    remote_close_name(&session).await?;

    Ok(())
}

async fn remote_close_env(session: &TestSession) -> anyhow::Result<()> {
    let tab = format!("remote_close_env/");

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 3000)
        .stdin("$TAB_BIN --close\n")
        .await_stdout("close", 300)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("env", result.snapshot);

    Ok(())
}

async fn remote_close_name(session: &TestSession) -> anyhow::Result<()> {
    let tab = format!("remote_close_name/");

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 3000)
        .stdin(format!("TAB_ID='' $TAB_BIN --close {}\n", tab))
        .await_stdout("close remote_close_name/", 300)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("name", result.snapshot);

    Ok(())
}
