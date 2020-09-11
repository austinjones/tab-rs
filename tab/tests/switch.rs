use insta::assert_snapshot;

mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
// #[tokio::test]
#[allow(dead_code)]
async fn switch() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("switch/a/")
        .await_stdout("$", 1000)
        .stdin("echo a\n")
        .await_stdout("echo a", 200)
        .await_stdout("$", 200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_a", result.stdout);

    let result = session
        .command()
        .tab("switch/b/")
        .await_stdout("$", 5000)
        .stdin("echo b\n")
        .await_stdout("echo b", 200)
        .await_stdout("$", 200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_b", result.stdout);

    let result = session
        .command()
        .tab("switch/a/")
        .await_stdout("$", 5000)
        .stdin("$TAB_BIN switch/b/\n")
        .await_stdout("echo b", 1000)
        .await_stdout("b", 100)
        .stdin("exit\n")
        .await_stdout("exit", 200)
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", result.stdout);

    Ok(())
}
