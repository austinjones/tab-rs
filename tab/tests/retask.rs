use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
#[tokio::test]
async fn retask() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    let result = session
        .command()
        .tab("target/")
        .await_stdout("$", 3000)
        .stdin("echo target\n")
        .await_stdout("echo target", 300)
        .await_stdout("$", 300)
        .complete_snapshot()
        .disconnect()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_target", result.stdout);

    for i in 0..5 {
        retask_iter(&session, i).await?;
    }

    Ok(())
}

async fn retask_iter(session: &TestSession, iter: usize) -> anyhow::Result<()> {
    let tab_from = format!("from/{}/", iter);

    let result = session
        .command()
        .tab(tab_from.as_str())
        .await_stdout("$", 3000)
        .stdin("echo from\n")
        .await_stdout("echo from", 300)
        .await_stdout("$", 300)
        .disconnect()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_from", result.snapshot);

    let result = session
        .command()
        .tab(tab_from)
        .await_stdout("$", 1000)
        .stdin("$TAB_BIN target/\n")
        .await_stdout("echo target", 1000)
        .await_stdout("target", 300)
        .await_stdout("$", 300)
        .complete_snapshot()
        .disconnect()
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("into_target", result.snapshot);

    Ok(())
}
