use std::{num::NonZeroUsize, sync::Arc, time::Instant};

use bytes::BytesMut;
use primitive::arena::obj_pool::{ArcObjPool, ObjScoped};
use tokio::{net::UdpSocket, task::JoinSet};
use udp_listener::{ConnRead, Packet};

use crate::{message::INIT_SIZE, scheduler::Stats};

#[derive(Debug)]
pub struct MpUdpRead {
    rx: tokio::sync::mpsc::Receiver<(usize, UdpRecvPkt)>,
    _recving: JoinSet<()>,
    stats: Stats,
    with_init: bool,
}
impl MpUdpRead {
    pub(crate) fn new(conns: Vec<UdpRecver>, stats: Stats, with_init: bool) -> Self {
        assert_eq!(conns.len(), stats.len());
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let mut recving = JoinSet::new();
        for (i, mut conn) in conns.into_iter().enumerate() {
            let tx = tx.clone();
            recving.spawn(async move {
                while let Some(pkt) = conn.recv().await {
                    if tx.send((i, pkt)).await.is_err() {
                        return;
                    }
                }
            });
        }
        Self {
            rx,
            _recving: recving,
            stats,
            with_init,
        }
    }
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, RecvError> {
        let (i, pkt) = self.rx.recv().await.ok_or(RecvError::Dead)?;
        let now = Instant::now();
        self.stats[i].lock().recv(now);
        let pkt = pkt.get();
        let payload = if self.with_init {
            &pkt[INIT_SIZE..]
        } else {
            pkt
        };
        let copy_len = buf.len().min(payload.len());
        buf[..copy_len].copy_from_slice(&payload[..copy_len]);
        Ok(copy_len)
    }
}
#[derive(Debug, Clone)]
pub enum RecvError {
    Dead,
}

#[derive(Debug)]
pub(crate) enum UdpRecver {
    Server(ConnRead<Packet>),
    Client(Arc<UdpSocket>, ArcObjPool<BytesMut>),
}
impl UdpRecver {
    pub fn from_client(socket: Arc<UdpSocket>) -> Self {
        pub const PACKET_BUFFER_LENGTH: usize = 2_usize.pow(16);
        const OBJ_POOL_SHARDS: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(4) };
        let pool = ArcObjPool::new(
            None,
            OBJ_POOL_SHARDS,
            || BytesMut::with_capacity(PACKET_BUFFER_LENGTH),
            |buf| buf.clear(),
        );
        Self::Client(socket, pool)
    }
    pub async fn recv(&mut self) -> Option<UdpRecvPkt> {
        match self {
            UdpRecver::Server(conn_read) => conn_read.recv().recv().await.map(UdpRecvPkt::Server),
            UdpRecver::Client(socket, pool) => {
                let mut buf = pool.take_scoped();
                if socket.recv_buf(&mut *buf).await.is_err() {
                    return None;
                };
                Some(UdpRecvPkt::Client(buf))
            }
        }
    }
}
#[derive(Debug)]
pub(crate) enum UdpRecvPkt {
    Server(Packet),
    Client(ObjScoped<BytesMut>),
}
impl UdpRecvPkt {
    pub fn get(&self) -> &[u8] {
        match self {
            UdpRecvPkt::Server(buf) => buf,
            UdpRecvPkt::Client(buf) => buf,
        }
    }
}