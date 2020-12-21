mod common;
use common::*;

/// Tests that two sessions can be established (A and B),
///  that A can be reconnected to,
///  and B can be switched to by executing the tab binary within A.
#[tokio::test]
async fn keyboard_latency() -> anyhow::Result<()> {
    let session = TestSession::new()?;

    // check that keyboard latency is under 5ms
    session
        .command()
        .strict_timeout()
        .tab("latency-test/")
        .await_stdout("$", 3000)
        .stdin("!!")
        .await_stdout("!!", 5)
        .stdin_bytes(&[0x14, 0x1b])
        .run()
        .await?;

    Ok(())
}
