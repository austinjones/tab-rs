/// Executes the task, until the future completes, or the lifeline is dropped
pub fn spawn<F: Future>(fut: F) -> Lifeline {
    let service = ServiceFuture::new(fut);
}

struct ServiceFuture<F: Future> {

}

impl<F: Future> ServiceFuture<F> {

}

impl<F: Future> Future for ServiceFuture<F> {
    
}

#[must_use]
struct Lifeline {

}

impl Drop for TaskLifeline {

}
pub struct ServiceSession {

}
#[async_trait]
pub trait Service {
    type IO;
    type Output = ();

    async fn start(io: Self::IO) -> Self;
    async fn shutdown(self) -> Self::Output;
}
pub struct TabService {
    name: String
}

pub struct TabServiceIO {
    pub rx: Receiver<Request>,
    pub tx: Sender<Response>
}

impl TabDriver {
    pub fn new(name: String) -> TabDriver {
        Self { name }
    }

    pub async fn run(self, rx: Receiver<Request>, tx: Sender<Response>) -> anyhow::Result<()> {
        tokio::task::spawn(forward_stdin(tx.clone()));

        recv_loop
    }
}

pub struct TabDriverState {
    pub selected_tab: Option<TabId>,
    pub awaiting_tab: Option<String>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            selected_tab: None,
            awaiting_tab: None,
        }
    }
}

async fn forward_stdin(mut tx: Sender<Request>) -> anyhow::Result<()> {
    let mut stdin = tokio::io::stdin();
    let mut buffer = vec![0u8; 512];

    while let Ok(read) = stdin.read(buffer.as_mut_slice()).await {
        if read == 0 {
            continue;
        }

        let mut buf = vec![0; read];
        buf.copy_from_slice(&buffer[0..read]);

        let chunk = InputChunk { data: buf };
        // TODO: use selected tab
        tx.send(Request::Input(TabId(0), chunk)).await?;
    }

    trace!("forward_stdin shutdown");

    Ok(())
}

async fn recv_loop(mut tx: Sender<Request>, mut rx: Receiver<Response>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = std::io::stdout();
    enable_raw_mode().expect("failed to enable raw mode");

    while let Some(message) = rx.recv().await {
        match message {
            Response::Output(_tab_id, chunk) => {
                let mut index = 0;
                for line in chunk.data.split(|e| *e == b'\n') {
                    stdout.write(line)?;

                    index += line.len();
                    if index < chunk.data.len() {
                        let next = chunk.data[index];

                        if next == b'\n' {
                            stdout.write("\r\n".as_bytes())?;
                            index += 1;
                        }
                    }
                }

                stdout.flush()?;
            }
            Response::TabUpdate(_tab) => {}
            Response::TabList(_tabs) => {}
            Response::TabTerminated(_tab) => {
                // TODO: filter to active tab
                break;
            }
            Response::Close => {}
        }
    }

    trace!("recv_loop shutdown");

    Ok(())
}

pub struct Actor<Recv, Send> {

}