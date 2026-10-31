//! Port série USB (crate `serialport`). Équivalent de LaserGRBL `ComWrapper/UsbSerial.cs`.

use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

use super::{LineBuffer, PortInfo, Transport, TransportError};

pub struct SerialTransport {
    name: String,
    port: Option<Box<dyn serialport::SerialPort>>,
    lines: LineBuffer,
}

impl SerialTransport {
    pub fn open(port: &str, baud: u32) -> Result<Self, TransportError> {
        let p = serialport::new(port, baud).timeout(Duration::from_millis(5)).open().map_err(|e| TransportError::Open(format!("{port} : {e}")))?;
        Ok(Self { name: format!("{port} @ {baud}"), port: Some(p), lines: LineBuffer::default() })
    }
}

impl Transport for SerialTransport {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let port = self.port.as_mut().ok_or(TransportError::Closed)?;
        port.write_all(data).and_then(|()| port.flush()).map_err(|e| TransportError::Io(e.to_string()))
    }

    fn read_line(&mut self, timeout: Duration) -> Result<Option<String>, TransportError> {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0u8; 512];
        loop {
            if let Some(line) = self.lines.pop_line() {
                return Ok(Some(line));
            }
            let port = self.port.as_mut().ok_or(TransportError::Closed)?;
            match port.read(&mut chunk) {
                Ok(0) => {}
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
        self.port = None;
        self.lines.clear();
    }
}

pub fn list_ports() -> Result<Vec<PortInfo>, TransportError> {
    let ports = serialport::available_ports().map_err(|e| TransportError::Io(e.to_string()))?;
    Ok(ports
        .into_iter()
        .map(|p| {
            let description = match p.port_type {
                serialport::SerialPortType::UsbPort(u) => {
                    let mut parts: Vec<String> = [u.manufacturer, u.product].into_iter().flatten().collect();
                    parts.push(format!("{:04X}:{:04X}", u.vid, u.pid));
                    parts.join(" — ")
                }
                serialport::SerialPortType::BluetoothPort => "Bluetooth".to_owned(),
                serialport::SerialPortType::PciPort => "PCI".to_owned(),
                serialport::SerialPortType::Unknown => String::new(),
            };
            PortInfo { name: p.port_name, description }
        })
        .collect())
}
