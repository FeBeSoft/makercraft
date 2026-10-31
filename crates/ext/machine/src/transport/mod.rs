//! Transports : comment les octets vont et viennent entre MakerCraft et la machine.
//!
//! Interface commune déduite de LaserGRBL 7.14.1 `ComWrapper/IComWrapper.cs`, Candle
//! `connections/connection.h` et UGS 2.1.27 `connection/ConnectionDriver.java` : ouvrir, écrire
//! des octets, lire des lignes, fermer. Le protocole (Grbl…) est au-dessus, dans `controller`.

use std::time::Duration;

pub mod emulator;
#[cfg(feature = "serial")]
pub mod serial;
pub mod tcp;

/// Erreur de transport.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("port introuvable ou occupé : {0}")]
    Open(String),
    #[error("erreur de communication : {0}")]
    Io(String),
    #[error("connexion fermée par la machine")]
    Closed,
    #[error("transport non disponible dans cette version : {0}")]
    Unsupported(&'static str),
}

/// Un lien bidirectionnel vers une machine.
pub trait Transport: Send {
    /// Description courte (`COM3 @ 115200`, `192.168.1.20:23`, `émulateur`).
    fn name(&self) -> String;
    /// Écrit tous les octets.
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError>;
    /// Renvoie la prochaine ligne complète reçue (sans fin de ligne, non vide), ou `None` si
    /// rien n'est arrivé dans le délai.
    fn read_line(&mut self, timeout: Duration) -> Result<Option<String>, TransportError>;
    /// Ferme le lien. Appels suivants : erreur `Closed`.
    fn close(&mut self);
}

/// Comment joindre la machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportConfig {
    /// Port série USB (`COM3`, `/dev/ttyUSB0`), 115200 bauds pour Grbl 1.1.
    Serial { port: String, baud: u32 },
    /// TCP/Telnet (ESP32, grblHAL réseau, FluidNC) : `hôte:port`.
    Tcp { address: String },
    /// Émulateur Grbl intégré, pour tester sans machine.
    Emulator,
}

impl TransportConfig {
    pub fn open(&self) -> Result<Box<dyn Transport>, TransportError> {
        match self {
            #[cfg(feature = "serial")]
            Self::Serial { port, baud } => Ok(Box::new(serial::SerialTransport::open(port, *baud)?)),
            #[cfg(not(feature = "serial"))]
            Self::Serial { .. } => Err(TransportError::Unsupported("port série")),
            Self::Tcp { address } => Ok(Box::new(tcp::TcpTransport::open(address)?)),
            Self::Emulator => Ok(Box::new(emulator::GrblEmulator::new())),
        }
    }
}

/// Découpe un flux d'octets en lignes (`\n`, `\r\n` ou `\r`), lignes vides ignorées.
/// Les octets non UTF-8 (bruit électrique) sont remplacés, jamais une cause d'erreur.
#[derive(Default, Debug)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

/// Au-delà, une « ligne » sans fin est forcément du bruit : on la coupe.
const MAX_LINE: usize = 4096;

impl LineBuffer {
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn pop_line(&mut self) -> Option<String> {
        loop {
            let end = match self.buf.iter().position(|&b| b == b'\n' || b == b'\r') {
                Some(i) => i,
                None if self.buf.len() > MAX_LINE => MAX_LINE,
                None => return None,
            };
            let line: Vec<u8> = self.buf.drain(..end).collect();
            if self.buf.first().is_some_and(|&b| b == b'\n' || b == b'\r') {
                self.buf.remove(0);
            }
            let text = String::from_utf8_lossy(&line).trim().to_owned();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

/// Un port série détecté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    /// Fabricant / produit / VID:PID quand c'est un adaptateur USB (CH340, FTDI, CP210x…).
    pub description: String,
}

/// Liste les ports série disponibles.
pub fn list_ports() -> Result<Vec<PortInfo>, TransportError> {
    #[cfg(feature = "serial")]
    {
        serial::list_ports()
    }
    #[cfg(not(feature = "serial"))]
    {
        Err(TransportError::Unsupported("port série"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_buffer_splits_any_line_ending() {
        let mut b = LineBuffer::default();
        b.push(b"ok\r\nerr");
        assert_eq!(b.pop_line().as_deref(), Some("ok"));
        assert_eq!(b.pop_line(), None);
        b.push(b"or:2\n\n\r<Idle>\rx\xff\n");
        assert_eq!(b.pop_line().as_deref(), Some("error:2"));
        assert_eq!(b.pop_line().as_deref(), Some("<Idle>"));
        assert_eq!(b.pop_line().as_deref(), Some("x\u{fffd}"));
        assert_eq!(b.pop_line(), None);
    }

    #[test]
    fn line_buffer_cuts_endless_noise() {
        let mut b = LineBuffer::default();
        b.push(&vec![b'a'; MAX_LINE + 10]);
        assert_eq!(b.pop_line().map(|l| l.len()), Some(MAX_LINE));
    }
}
