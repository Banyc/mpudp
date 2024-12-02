use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::net::UdpSocket;
use udp_listener::{ConnWrite, PACKET_BUFFER_LENGTH};

use crate::{
    message::{Header, Init},
    scheduler::{Rank, Stats},
};

const RANK_UPDATE_COOL_DOWN: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct MpUdpWrite {
    stats: Stats,
    rank: Rank,
    last_rank_update: Instant,
    conns: Vec<UdpSender>,
    init: Option<Init>,
    buf: Vec<u8>,
}
impl MpUdpWrite {
    pub(crate) fn new(conns: Vec<UdpSender>, stats: Stats, init: Option<Init>) -> Self {
        assert_eq!(conns.len(), stats.len());
        let now = Instant::now();
        let rank = Rank::new(conns.len());
        let buf = Vec::with_capacity(PACKET_BUFFER_LENGTH);
        Self {
            conns,
            stats,
            rank,
            last_rank_update: now,
            init,
            buf,
        }
    }
    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        let now = Instant::now();
        if RANK_UPDATE_COOL_DOWN < now.duration_since(self.last_rank_update) {
            let stats = self.stats.iter().map(|s| *s.lock()).collect::<Vec<_>>();
            self.rank.update_rank(stats.iter(), now);
            self.last_rank_update = now;
        }
        let buf = match &self.init {
            Some(init) => {
                self.buf.clear();
                let with_payload = true;
                let header = Header::new(*init, with_payload);
                self.buf.extend(header.encode());
                self.buf.extend(buf);
                &self.buf
            }
            None => buf,
        };
        let exploit = self.rank.choose_exploit().unwrap();
        let explore = self.rank.choose_explore(exploit);
        if let Some(explore) = explore {
            self.stats[explore].lock().sent(now);
            self.conns[explore].send(buf).await?;
        }
        let n = self.conns[exploit].send(buf).await?;
        self.stats[exploit].lock().sent(now);
        Ok(n)
    }
}

#[derive(Debug)]
pub(crate) enum UdpSender {
    Server(ConnWrite<UdpSocket>),
    Client(Arc<UdpSocket>),
}
impl UdpSender {
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        match &self {
            UdpSender::Server(conn_write) => conn_write.send(buf).await,
            UdpSender::Client(socket) => socket.send(buf).await,
        }
    }
}
