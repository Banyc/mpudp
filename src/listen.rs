use std::{
    io::{self, Read},
    net::SocketAddr,
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

use tokio::{net::UdpSocket, task::JoinSet};
use udp_listener::{Packet, UtpListener};

use crate::{
    backlog::Backlog,
    conn::MpUdpConn,
    message::{HEADER_SIZE, Header},
    read::{MpUdpRead, UdpRecver},
    scheduler::new_stats,
    write::{MpUdpWrite, UdpSender},
};

const BACKLOG_TIMEOUT: Duration = Duration::from_secs(60);
const BACKLOG_MAX: usize = 64;

#[derive(Debug)]
pub struct MpUdpListener {
    listeners: Vec<Listener>,
    complete: tokio::sync::mpsc::Receiver<io::Result<MpUdpConn>>,
    _backlog_handling: JoinSet<()>,
}
impl MpUdpListener {
    pub async fn bind(
        addrs: impl Iterator<Item = SocketAddr>,
        max_session_conns: NonZeroUsize,
        dispatcher_buffer_size: NonZeroUsize,
    ) -> io::Result<Self> {
        let mut listeners = vec![];
        for addr in addrs {
            let socket = UdpSocket::bind(addr).await?;
            let addr = socket.local_addr().unwrap();
            let listener = UtpListener::new_identity_dispatch(socket, dispatcher_buffer_size);
            listeners.push(Listener {
                listener: Arc::new(listener),
                local_addr: addr,
            });
        }
        if listeners.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "number of addresses cannot be zero",
            ));
        }
        let backlog = Backlog::new(max_session_conns);
        let backlog = Arc::new(backlog);
        let mut backlog_handling = JoinSet::new();
        backlog_handling.spawn({
            let backlog = Arc::clone(&backlog);
            async move {
                loop {
                    tokio::time::sleep(BACKLOG_TIMEOUT / 2).await;
                    backlog.clean(BACKLOG_TIMEOUT);
                }
            }
        });
        let (tx, rx) = tokio::sync::mpsc::channel(BACKLOG_MAX);
        for listener in &listeners {
            let listener = Arc::clone(&listener.listener);
            let backlog = Arc::clone(&backlog);
            let complete = tx.clone();
            backlog_handling.spawn(async move {
                loop {
                    let mut conn = match listener.accept().await {
                        Ok(x) => x,
                        Err(e) => {
                            if complete.send(Err(e)).await.is_err() {
                                break;
                            }
                            continue;
                        }
                    };
                    let pkt = conn.read().recv().recv().await.unwrap();
                    let mut rdr = io::Cursor::new(&pkt[..]);
                    let mut header = [0; HEADER_SIZE];
                    if rdr.read_exact(&mut header).is_err() {
                        continue;
                    }
                    let Ok(header) = Header::decode(header) else {
                        continue;
                    };
                    let session = header.init().session();
                    let conns = header.init().conns();
                    if max_session_conns < conns {
                        continue;
                    }
                    let Some(conns) = backlog.handle(session, conn, conns) else {
                        continue;
                    };
                    let stats = new_stats(conns.len());
                    let mut read = vec![];
                    let mut write = vec![];
                    for conn in conns {
                        let (r, w) = conn.split();
                        let r = UdpRecver::Server(r);
                        read.push(r);
                        let w = UdpSender::Server(w);
                        write.push(w);
                    }
                    let with_header = true;
                    let read = MpUdpRead::new(read, stats.clone(), with_header);
                    let write = MpUdpWrite::new(write, stats, None);
                    let conn = MpUdpConn::new(read, write);
                    if complete.send(Ok(conn)).await.is_err() {
                        break;
                    }
                }
            });
        }
        Ok(Self {
            listeners,
            complete: rx,
            _backlog_handling: backlog_handling,
        })
    }

    /// # Cancel safety
    ///
    /// This method is cancel safe. If `accept` is used as the event in a `tokio::select` statement and some other branch completes first, it is guaranteed that no streams were received on this listener.
    pub async fn accept(&mut self) -> io::Result<MpUdpConn> {
        self.complete
            .recv()
            .await
            .expect("senders will never drop proactively")
    }

    pub fn local_addrs(&self) -> impl Iterator<Item = SocketAddr> + '_ {
        self.listeners.iter().map(|listener| listener.local_addr)
    }
}

#[derive(Debug)]
struct Listener {
    pub listener: Arc<UtpListener<UdpSocket, SocketAddr, Packet>>,
    pub local_addr: SocketAddr,
}
