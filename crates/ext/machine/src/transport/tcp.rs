//! TCP / Telnet (cartes ESP32, grblHAL réseau, FluidNC). Équivalent de LaserGRBL
//! `ComWrapper/Telnet.cs` et Candle `connections/telnetconnection.cpp`.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use super::{LineBuffer, Transport, TransportError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TcpTransport {
    name: String,
    stream: Option<TcpStream>,
    lines: LineBuffer,
}

impl TcpTransport {
    /// `address` : `hôte:port` (port 23 si absent).
    pub fn open(address: &str) -> Result<Self, TransportError> {
        let full = if address.contains(':') { address.to_owned() } else { format!("{address}:23") };
        let addr = full
            .to_socket_addrs()
            .map_err(|e| TransportError::Open(format!("{full} : {e}")))?
            .next()
            .ok_or_else(|| TransportError::Open(format!("{full} : adresse introuvable")))?;
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| TransportError::Open(format!("{full} : {e}")))?;
        let _ = stream.set_nodelay(true);
        stream.set_read_timeout(Some(Duration::from_millis(5))).map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(Self { name: full, stream: Some(stream), lines: LineBuffer::default() })
    }
}

impl Transport for TcpTransport {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let s = self.stream.as_mut().ok_or(TransportError::Closed)?;
        s.write_all(data).map_err(|e| TransportError::Io(e.to_string()))
    }

    fn read_line(&mut self, timeout: Duration) -> Result<Option<String>, TransportError> {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0u8; 512];
        loop {
            if let Some(line) = self.lines.pop_line() {
                return Ok(Some(line));
            }
            let s = self.stream.as_mut().ok_or(TransportError::Closed)?;
            match s.read(&mut chunk) {
                Ok(0) => {
                    self.stream = None;
                    return Err(TransportError::Closed);
                }
                Ok(n) => self.lines.push(chunk.get(..n).unwrap_or(&[])),
                Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(e) => return Err(TransportError::Io(e.to_string())),
            }
            if Instant::now() >= deadline {
                return Ok(self.lines.pop_line());
            }
        }
    }

    fn close(&mut self) {
        if let Some(s) = self.stream.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        self.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn talks_to_a_local_server() {
        let Ok(listener) = TcpListener::bind("127.0.0.1:0") else { return };
        let Ok(addr) = listener.local_addr() else { return };
        let server = std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = s.write_all(b"Grbl 1.1h ['$' for help]\r\n");
                let mut buf = [0u8; 16];
                let n = s.read(&mut buf).unwrap_or(0);
                let _ = s.write_all(if buf.get(..n) == Some(b"?") { b"<Idle|MPos:0,0,0>\r\n" } else { b"error\r\n" });
            }
        });
        let mut t = match TcpTransport::open(&addr.to_string()) {
            Ok(t) => t,
            Err(e) => panic!("{e}"),
        };
        assert_eq!(t.read_line(Duration::from_secs(2)), Ok(Some("Grbl 1.1h ['$' for help]".into())));
        assert_eq!(t.write(b"?"), Ok(()));
        assert_eq!(t.read_line(Duration::from_secs(2)), Ok(Some("<Idle|MPos:0,0,0>".into())));
        let _ = server.join();
        assert_eq!(t.read_line(Duration::from_secs(2)), Err(TransportError::Closed));
    }
}
