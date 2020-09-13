use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
#[tokio::test]
async fn session() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("target/")
        .await_stdout("$", 5000)
        .stdin("echo target\n")
        .await_stdout("echo target", 200)
        .await_stdout("$", 200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_target", result.stdout);

    for i in 0..5 {
        session_iter(&mut session, i).await?;
    }

    Ok(())
}

async fn session_iter(session: &mut TestSession, iter: usize) -> anyhow::Result<()> {
    let tab_from = format!("from/{}/", iter);

    let result = session
        .command()
        .tab(tab_from.as_str())
        .await_stdout("$", 1000)
        .stdin("echo from\n")
        .await_stdout("echo from", 200)
        .await_stdout("$", 200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_from", result.stdout);

    let result = session
        .command()
        .tab(tab_from)
        .await_stdout("$", 5000)
        .stdin("$TAB_BIN target/\n")
        .await_stdout("echo target", 1000)
        .await_stdout("target", 200)
        .await_stdout("$", 200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("into_target", result.stdout);

    Ok(())
}
