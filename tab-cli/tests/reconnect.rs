use insta::assert_snapshot;

mod common;
use common::*;

#[tokio::test]
async fn test_reconnect() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("reconnect/")
        .delay_ms(1000)
        .stdin("echo foo\n")
        .delay_ms(200) // delay a few moment, so we can confirm `echo foo` is echoed
        .stdin_bytes(&[23u8])
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("before", result.stdout);

    let result = session.command().stdin("exit\n").run().await?;
    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("after", result.stdout);

    Ok(())
}