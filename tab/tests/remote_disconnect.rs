use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
#[tokio::test]
async fn remote_disconnect() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    remote_disconnect_env(&session).await?;

    remote_disconnect_name(&session).await?;

    Ok(())
}

async fn remote_disconnect_env(session: &TestSession) -> anyhow::Result<()> {
    let tab = format!("remote_disconnect_env/");

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 3000)
        .stdin("$TAB_BIN --disconnect\n")
        .await_stdout("disconnect", 300)
        // TODO: extend snapshot to cover the message printed by tab bin
        // there is a race condition that makes it sometimes not appear
        // same for the remote_close test
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("env", result.snapshot);

    let tabs = session.command().tabs().await?;
    assert_eq!(vec!["remote_disconnect_env/".to_string()], tabs);

    Ok(())
}

async fn remote_disconnect_name(session: &TestSession) -> anyhow::Result<()> {
    let tab = format!("remote_disconnect_name/");

    let result = session
        .command()
        .tab(tab.as_str())
        .await_stdout("$", 1000)
        .stdin(format!("TAB_ID='' $TAB_BIN --disconnect {}\n", tab))
        .await_stdout("disconnect remote_disconnect_name/", 300)
        .complete_snapshot()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("name", result.snapshot);

    let tabs = session.command().tabs().await?;
    assert_eq!(
        vec![
            "remote_disconnect_env/".to_string(),
            "remote_disconnect_name/".to_string()
        ],
        tabs
    );

    Ok(())
}
