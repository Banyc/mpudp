use std::{io, sync::Arc, time::Duration};

use tokio::{net::UdpSocket, task::JoinSet};
use udp_listener::{Conn, Packet, UtpListener};

use crate::message::Session;

const BACKLOG_TIMEOUT: Duration = Duration::from_secs(60);
const INIT_TIMEOUT: Duration = Duration::from_secs(1);
const BACKLOG_MAX: usize = 64;

#[derive(Debug)]
pub struct MpUdpListener {
    listeners: Vec<Arc<UtpListener<UdpSocket, Session, Packet>>>,
    complete: tokio::sync::mpsc::Receiver<io::Result<Conn<UdpSocket, Session, Packet>>>,
    backlog_handling: JoinSet<()>,
}
