use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::str::Split;
use std::time::{Duration, Instant};

static PORT: u16 = 4000;
static LOCAL_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 2); // TODO: make this configurable
static REMOTE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 1); // TODO: make this configurable

static POLL_DELAY: Duration = Duration::from_millis(1); // prevent interface spam

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
    polled_amt: u32,
    started: Instant,
    initialized: bool,
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
            polled_amt: 0,
            started: Instant::now(),
            initialized: false,
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

    pub fn connect_socket(&mut self) -> Result<(), ViewerBackendError> {
        self.socket
            .connect(SocketAddr::from((REMOTE_IP, PORT)))
            .map_err(|e| ViewerBackendError::SocketError(e))?;
        Ok(())
    }

    /// poll new values or reads cached ones if delay has not yet elapsed
    pub fn poll(&mut self) -> Result<&AnalogValues, ViewerBackendError> {
        if self.last_poll.elapsed() < POLL_DELAY {
            return Ok(&self.analog_vals);
        }

        if !self.initialized {
            self.socket
                .send_to(b"init", SocketAddr::from((REMOTE_IP, PORT)))
                .map_err(|e| ViewerBackendError::SocketError(e))?;
            self.initialized = true;
        }

        log::info!("polling");

        let mut buf = [0u8; 8];

        self.socket
            .send_to(b"poll", SocketAddr::from((REMOTE_IP, PORT)))
            .map_err(|e| ViewerBackendError::SocketError(e))?;

        let amt = match self.socket.recv(&mut buf) {
            Ok(amt) => amt,
            Err(e) => {
                self.socket
                    .connect(SocketAddr::from((REMOTE_IP, PORT)))
                    .map_err(|e| ViewerBackendError::SocketError(e))?;

                Err(ViewerBackendError::SocketError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))?
            }
        };
        log::info!("amt: {}", amt);

        let mut values: [u16; 4] = [0, 0, 0, 0];

        let mut offs = 0;
        for i in 0..4 {
            values[i] = u16::from_be_bytes([buf[offs], buf[offs + 1]]);
            offs += 2;
        }

        self.analog_vals = AnalogValues {
            a0: values[0],
            a1: values[1],
            a2: values[2],
            a3: values[3],
        };

        log::info!("analog_vals: {:?}", self.analog_vals);

        self.last_poll = Instant::now();
        self.polled_amt += 1;

        log::info!("polled {} times", self.polled_amt);
        log::info!("started {} ms ago", self.started.elapsed().as_millis());

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
