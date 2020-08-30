use anyhow::Context;
use insta::{assert_debug_snapshot, assert_snapshot};
use lifeline::assert_completes;
use std::process::Stdio;
use tempfile::tempdir;

use common::*;
use time::Duration;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, time};

mod common;

#[tokio::test]
async fn session() -> anyhow::Result<()> {
    let mut session = TestSession::new()?;

    let result = session
        .command()
        .tab("session/")
        .delay_ms(1000)
        .stdin("echo foo\n")
        .stdin("exit\n")
        .run()
        .await?;

    assert_eq!(Some(0), result.exit_status.code());
    assert_snapshot!("result", result.stdout);

    Ok(())
}
