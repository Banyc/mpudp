use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
};

// #[derive(Debug, Clone)]
// pub enum OptionalInit {
//     None,
//     Some(Init),
// }

pub const HEADER_SIZE: usize = INIT_SIZE + 1;
pub type HeaderBuf = [u8; HEADER_SIZE];
#[derive(Debug, Clone, Copy)]
pub struct Header {
    init: Init,
    with_payload: bool,
}
impl Header {
    pub fn new(init: Init, with_payload: bool) -> Self {
        Self { init, with_payload }
    }
    pub fn init(&self) -> &Init {
        &self.init
    }
    pub fn with_payload(&self) -> bool {
        self.with_payload
    }

    pub fn encode(&self) -> HeaderBuf {
        let mut buf = [0; HEADER_SIZE];
        let mut wtr = io::Cursor::new(&mut buf[..]);
        let init = self.init.encode();
        wtr.write_all(&init[..]).unwrap();
        match self.with_payload {
            true => wtr.write_all(&[1]).unwrap(),
            false => wtr.write_all(&[0]).unwrap(),
        }
        buf
    }
    pub fn decode(buf: HeaderBuf) -> io::Result<Self> {
        let mut rdr = io::Cursor::new(&buf[..]);
        let mut init = [0; INIT_SIZE];
        rdr.read_exact(&mut init).unwrap();
        let init = Init::decode(init)?;
        let mut with_payload = [0];
        rdr.read_exact(&mut with_payload).unwrap();
        let with_payload = with_payload[0];
        let with_payload = match with_payload {
            0 => false,
            1 => true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown with_payload: {with_payload}"),
                ));
            }
        };
        Ok(Self { init, with_payload })
    }
}

pub const INIT_SIZE: usize = 8 * 2;
pub type InitBuf = [u8; INIT_SIZE];
#[derive(Debug, Clone, Copy)]
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
        let mut rdr = io::Cursor::new(&buf[..]);
        let mut session = 0_u64.to_be_bytes();
        rdr.read_exact(&mut session).unwrap();
        let session = Session::new(u64::from_be_bytes(session));
        let mut conns = 0_u64.to_be_bytes();
        rdr.read_exact(&mut conns).unwrap();
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
