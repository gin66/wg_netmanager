use std::net::{UdpSocket, ToSocketAddrs, SocketAddr};

use crate::error::*;

pub struct CryptUdp {
    socket: UdpSocket,
    key: Option<[u8; 32]>,
}

impl CryptUdp {
    pub fn bind(port: u16) -> BoxResult<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
        Ok(CryptUdp {
            socket,
            key: None,
        })
    }
    pub fn key(mut self, key: &[u8]) -> BoxResult<Self> {
        if key.len() != 32 {
            Err("Invalid key length")?
        }
        else {
            let mut key_buf: [u8; 32] = Default::default();
            key_buf.copy_from_slice(key);
            self.key = Some(key_buf);
            Ok(self)
        }
    }
    pub fn try_clone(&self) -> BoxResult<Self> {
        Ok(CryptUdp {
            socket: self.socket.try_clone()?,
            key: self.key.clone(),
        })
    }
    pub fn send_to<T: ToSocketAddrs>(&self, buf: &[u8], addr: T) -> BoxResult<usize> {
        if let Some(key) = self.key.as_ref() {
            Ok(self.socket.send_to(buf, addr)?)
        }
        else {
            Err("No encryption key")?
        }
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> BoxResult<(usize, SocketAddr)> {
        if let Some(key) = self.key.as_ref() {
            Ok(self.socket.recv_from(buf)?)
        }
        else {
            Err("No encryption key")?
        }
    }
}
