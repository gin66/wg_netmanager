use std::net::{UdpSocket, ToSocketAddrs, SocketAddr};

use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce};
use chacha20poly1305::aead::{Aead, NewAead};

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
        if let Some(raw_key) = self.key.as_ref() {
            let nonce_raw: [u8; 24]  = rand::random(); 
            let nonce = XNonce::from_slice(&nonce_raw);
            let key = Key::from_slice(raw_key);
            let cipher = XChaCha20Poly1305::new(key.into());
            let mut encrypted = cipher.encrypt(nonce, buf).map_err(|e| format!("{:?}",e))?;
            encrypted.append(&mut nonce_raw.to_vec());
            Ok(self.socket.send_to(&encrypted, addr)?)
        }
        else {
            Err("No encryption key")?
        }
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> BoxResult<(usize, SocketAddr)> {
        if let Some(raw_key) = self.key.as_ref() {
            let mut enc_buf: Vec<u8> = vec![0; 1500];
            let (length, src_addr) = self.socket.recv_from(&mut enc_buf)?;

            if length <= 24 {
                Err("received buffer too short")?;
            }
            let new_length = length - 24;


            let nonce_raw = enc_buf[new_length .. length].to_vec();
            let nonce = XNonce::from_slice(&nonce_raw);
            let key = Key::from_slice(raw_key);
            let cipher = XChaCha20Poly1305::new(key.into());
            let decrypted = cipher.decrypt(nonce, &enc_buf[.. new_length]).map_err(|e| format!("{:?}",e))?;

            buf[.. decrypted.len()].copy_from_slice(&decrypted);

            Ok((decrypted.len(), src_addr))
        }
        else {
            Err("No encryption key")?
        }
    }
}
