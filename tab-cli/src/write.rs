use futures::{Future, Stream};
use tab_api::response::Response;

async fn recv_loop(
    mut rx: impl Stream<Item = impl Future<Output = anyhow::Result<Response>>> + Unpin,
) -> anyhow::Result<()> {
    info!("Waiting on messages...");

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    // enable_raw_mode().expect("failed to enable raw mode");

    while let Some(message) = rx.next().await {
        let message = message.await?;
        // info!("message: {:?}", message);

        match message {
            Response::Chunk(tab_id, chunk) => match chunk.channel {
                ChunkType::Stdout => {
                    for line in chunk.data.split(|e| *e == b'\n') {
                        stdout.write(line).await?;
                        stdout.write(&[b'\n']).await?;
                    }
                }
                ChunkType::Stderr => {
                    // stderr.write_all(chunk.data.as_slice()).await?;
                }
            },
            Response::TabUpdate(tab) => {}
            Response::TabList(tabs) => {}
        }
    }

    Ok(())
}
