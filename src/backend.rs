use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::str::Split;
use std::time::{Duration, Instant};

static PORT: u16 = 4000;
static LOCAL_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 2); // TODO: make this configurable
static REMOTE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 1); // TODO: make this configurable

static POLL_DELAY: Duration = Duration::from_millis(5); // prevent interface spam

#[derive(Debug)]
pub struct AnalogValues {
    pub a0: u16,
    pub a1: u16,
    pub a2: u16,
    pub a3: u16,
}

pub struct ViewerBackend {
    socket: UdpSocket,
    analog_vals: AnalogValues,
    last_poll: Instant,
}

#[derive(Debug)]
pub enum ViewerBackendError {
    SocketError(std::io::Error),
    ParserError(String), // reason
}

impl From<ViewerBackendError> for std::io::Error {
    fn from(e: ViewerBackendError) -> Self {
        match e {
            ViewerBackendError::SocketError(e) => e,
            ViewerBackendError::ParserError(s) => std::io::Error::new(std::io::ErrorKind::Other, s),
        }
    }
}

impl From<ViewerBackendError> for std::fmt::Error {
    fn from(_: ViewerBackendError) -> Self {
        std::fmt::Error
    }
}

impl ViewerBackend {
    /// connect to the device so we can poll values
    /// the device expects us to poll it often, otherwise it needs to be restarted
    pub fn connect() -> Result<Self, ViewerBackendError> {
        let socket = UdpSocket::bind(SocketAddr::from((LOCAL_IP, PORT)))
            .map_err(|e| ViewerBackendError::SocketError(e))?;

        Ok(ViewerBackend {
            socket,
            analog_vals: AnalogValues {
                a0: 0,
                a1: 0,
                a2: 0,
                a3: 0,
            },
            last_poll: Instant::now(),
        })
    }

    /// parse a piece of the message (split by :)
    fn parse_piece(split: &mut Split<'_, char>, name: &str) -> Result<u16, ViewerBackendError> {
        Ok(split
            .next()
            .ok_or(ViewerBackendError::ParserError(format!(
                "missing {} value",
                name
            )))?
            .parse()
            .map_err(|e| {
                ViewerBackendError::ParserError(format!("invalid {} value: {:?}", name, e))
            })?)
    }

    /// reads analog vals without updating them
    /// helpful if &mut self is not available
    pub fn read(&self) -> Result<&AnalogValues, ViewerBackendError> {
        match self.analog_vals {
            AnalogValues {
                a0: 0,
                a1: 0,
                a2: 0,
                a3: 0,
            } => Err(ViewerBackendError::ParserError(String::from(
                "no values read yet",
            )))?,
            _ => Ok(&self.analog_vals),
        }
    }

    /// poll new values or reads cached ones if delay has not yet elapsed
    pub fn poll(&mut self) -> Result<&AnalogValues, ViewerBackendError> {
        if self.last_poll.elapsed() < POLL_DELAY {
            return Ok(&self.analog_vals);
        }

        log::info!("polling");
        let mut buf = [0u8; 1024];

        self.socket
            .connect(SocketAddr::from((REMOTE_IP, PORT)))
            .map_err(|e| ViewerBackendError::SocketError(e))?;
        self.socket
            .send(&[0u8])
            .map_err(|e| ViewerBackendError::SocketError(e))?;
        let (amt, src) = match self.socket.recv_from(&mut buf) {
            Ok((amt, src)) => (amt, src),
            Err(_) => Err(ViewerBackendError::SocketError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "recv_from failed",
            )))?,
        };
        log::info!("amt: {}, src: {}", amt, src);

        let recv_str = std::str::from_utf8(&buf[..amt])
            .map_err(|_| ViewerBackendError::ParserError(String::from("invalid utf8")))?;

        let recv_str = recv_str.trim();

        log::info!("recv: {:?}", recv_str);

        let mut pieces = recv_str.split(':');

        self.analog_vals = AnalogValues {
            a0: Self::parse_piece(&mut pieces, "a0")?,
            a1: Self::parse_piece(&mut pieces, "a1")?,
            a2: Self::parse_piece(&mut pieces, "a2")?,
            a3: Self::parse_piece(&mut pieces, "a3")?,
        };

        log::info!("analog_vals: {:?}", self.analog_vals);

        self.last_poll = Instant::now();

        Ok(&self.analog_vals)
    }
}

/// poll values and display them in a human readable format
impl std::fmt::Display for ViewerBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vals = match self.read() {
            Ok(vals) => vals,
            Err(e) => {
                log::error!("poll failed: {:?}", e);
                return Err(e.into());
            }
        };

        write!(
            f,
            "(a0: {}, a1: {}, a2: {}, a3: {})",
            vals.a0, vals.a1, vals.a2, vals.a3
        )
    }
}

/// see impl of Display for details about this implementation
impl std::fmt::Debug for ViewerBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (self as &dyn std::fmt::Display).fmt(f)
    }
}
