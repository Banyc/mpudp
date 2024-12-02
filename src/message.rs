use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
};

// #[derive(Debug, Clone)]
// pub enum OptionalInit {
//     None,
//     Some(Init),
// }

pub const INIT_SIZE: usize = 8 * 2;
pub type InitBuf = [u8; INIT_SIZE];
#[derive(Debug, Clone)]
pub struct Init {
    session: Session,
    conns: NonZeroUsize,
}
impl Init {
    pub fn new(session: Session, conns: NonZeroUsize) -> Self {
        Self { session, conns }
    }
    pub fn session(&self) -> Session {
        self.session
    }
    pub fn conns(&self) -> NonZeroUsize {
        self.conns
    }

    pub fn encode(&self) -> InitBuf {
        let mut buf = [0; INIT_SIZE];
        let mut wtr = io::Cursor::new(&mut buf[..]);
        wtr.write_all(&self.session.inner().to_be_bytes()).unwrap();
        wtr.write_all(&(self.conns.get() as u64).to_be_bytes())
            .unwrap();
        buf
    }
    pub fn decode(buf: InitBuf) -> io::Result<Self> {
        let mut buf_reader = io::Cursor::new(&buf[..]);

        let mut session = 0_u64.to_be_bytes();
        buf_reader.read_exact(&mut session).unwrap();
        let session = Session::new(u64::from_be_bytes(session));
        let mut conns = 0_u64.to_be_bytes();
        buf_reader.read_exact(&mut conns).unwrap();
        let conns = usize::try_from(u64::from_be_bytes(conns))
            .map_err(|e| io::Error::new(io::ErrorKind::Unsupported, e))?;
        let conns = NonZeroUsize::new(conns)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "zero conns"))?;
        Ok(Self { session, conns })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub struct Session(u64);
impl Session {
    pub fn random() -> Self {
        Self::new(rand::random())
    }
    pub fn new(number: u64) -> Self {
        Self(number)
    }
    pub fn inner(&self) -> u64 {
        self.0
    }
}
