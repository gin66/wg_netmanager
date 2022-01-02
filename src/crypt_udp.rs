use std::net::{UdpSocket, ToSocketAddrs, SocketAddr};
use std::io::Result;

pub struct CryptUdp {
    socket: UdpSocket,
}

impl CryptUdp {
    pub fn bind(port: u16) -> Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
        Ok(CryptUdp {
            socket,
        })
    }
    pub fn try_clone(&self) -> Result<Self> {
        Ok(CryptUdp {
            socket: self.socket.try_clone()?,
        })
    }
    pub fn send_to<T: ToSocketAddrs>(&self, buf: &[u8], addr: T) -> Result<()> {
        self.socket.send_to(buf, addr)?;
        Ok(())
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }
}
