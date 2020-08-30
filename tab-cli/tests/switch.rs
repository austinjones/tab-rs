use insta::assert_snapshot;

mod common;
use common::*;

// #[tokio::test]
async fn switch() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("switch/a/")
        .delay_ms(1000)
        .stdin("echo a\n")
        .delay_ms(200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_a", result.stdout);

    let result = session
        .command()
        .tab("switch/b/")
        .delay_ms(1000)
        .stdin("echo b\n")
        .delay_ms(200)
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("create_b", result.stdout);

    let result = session
        .command()
        .tab("switch/a/")
        .delay_ms(1000)
        .stdin("$TAB_BIN switch/b/\n")
        .delay_ms(200)
        .stdin("exit\n")
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", result.stdout);

    Ok(())
}
