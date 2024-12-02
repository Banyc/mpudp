use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZeroUsize,
    sync::Arc,
};

use tokio::net::UdpSocket;

use crate::{
    message::{Header, Init, Session},
    read::{MpUdpRead, UdpRecver},
    scheduler::new_stats,
    write::{MpUdpWrite, UdpSender},
};

#[derive(Debug)]
pub struct MpUdpConn {
    write: MpUdpWrite,
    read: MpUdpRead,
}
impl MpUdpConn {
    pub(crate) fn new(read: MpUdpRead, write: MpUdpWrite) -> Self {
        Self { write, read }
    }
    pub fn split_mut(&mut self) -> (&mut MpUdpRead, &mut MpUdpWrite) {
        (&mut self.read, &mut self.write)
    }
    pub fn into_split(self) -> (MpUdpRead, MpUdpWrite) {
        (self.read, self.write)
    }
    pub async fn connect(addrs: impl Iterator<Item = SocketAddr>) -> io::Result<Self> {
        let mut sockets = vec![];
        for addr in addrs {
            let any = match addr {
                SocketAddr::V4(_) => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
                SocketAddr::V6(_) => {
                    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))
                }
            };
            let socket = UdpSocket::bind(any).await?;
            socket.connect(addr).await?;
            sockets.push(Arc::new(socket));
        }
        let conns = NonZeroUsize::new(sockets.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "zero addresses"))?;
        let session = Session::random();
        let init = Init::new(session, conns);
        let with_payload = false;
        let header = Header::new(init, with_payload);
        let header = header.encode();
        for socket in &sockets {
            socket.send(&header).await?;
        }
        let mut write = vec![];
        let mut read = vec![];
        let stats = new_stats(sockets.len());
        for socket in sockets {
            let sender = UdpSender::Client(Arc::clone(&socket));
            write.push(sender);
            let recver = UdpRecver::from_client(socket);
            read.push(recver);
        }
        let read_with_init = false;
        let read = MpUdpRead::new(read, stats.clone(), read_with_init);
        let write = MpUdpWrite::new(write, stats, Some(init));
        Ok(Self { write, read })
    }
}
