use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::SystemTime;

use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use crc::Crc;
use log::*;

use crate::error::*;

// Udp-Packet structure:
//   n Bytes   Encrypted data
//  24 Bytes   Nonce
//
// Encrypted data:
//   p Bytes   Paylod
//   ? bytes   padding to 8*x+2
//   2 Bytes   Length of Payload
//             ----- padded here to 8*x
//   8 Bytes   Timestamp
//   8 Bytes   CRC

pub struct CryptUdp {
    socket: UdpSocket,
    key: Option<[u8; 32]>,
}

impl CryptUdp {
    pub fn bind(port: u16) -> BoxResult<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
        Ok(CryptUdp { socket, key: None })
    }
    pub fn key(mut self, key: &[u8]) -> BoxResult<Self> {
        if key.len() != 32 {
            strerror("Invalid key length")?
        } else {
            let mut key_buf: [u8; 32] = Default::default();
            key_buf.copy_from_slice(key);
            self.key = Some(key_buf);
            Ok(self)
        }
    }
    pub fn try_clone(&self) -> BoxResult<Self> {
        Ok(CryptUdp {
            socket: self.socket.try_clone()?,
            key: self.key,
        })
    }
    pub fn send_to<T: ToSocketAddrs>(&self, payload: &[u8], addr: T) -> BoxResult<usize> {
        if let Some(raw_key) = self.key.as_ref() {
            let p = payload.len();
            let padded = ((p + 2 + 7) / 8) * 8; // +2 for 2 Byte length
            let enc_length = padded + 16;

            let timestamp: u64 = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut buf = vec![0u8; enc_length];
            buf[..p].copy_from_slice(payload);
            buf[padded - 2..padded].copy_from_slice(&(p as u16).to_le_bytes());
            buf[padded..padded + 8].copy_from_slice(&timestamp.to_le_bytes());

            let crc_gen = Crc::<u64>::new(&crc::CRC_64_ECMA_182);
            let mut digest = crc_gen.digest();
            digest.update(&buf[..padded + 8]);
            let crc_result = digest.finalize();

            buf[padded + 8..padded + 16].copy_from_slice(&crc_result.to_le_bytes());

            let nonce_raw: [u8; 24] = rand::random();
            let nonce = XNonce::from_slice(&nonce_raw);
            let key = Key::from_slice(raw_key);
            let cipher = XChaCha20Poly1305::new(key);
            let mut encrypted = cipher
                .encrypt(nonce, &buf[..])
                .map_err(|e| format!("{:?}", e))?;
            encrypted.append(&mut nonce_raw.to_vec());
            Ok(self.socket.send_to(&encrypted, addr)?)
        } else {
            strerror("No encryption key")?
        }
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> BoxResult<(usize, SocketAddr)> {
        if let Some(raw_key) = self.key.as_ref() {
            let mut enc_buf: Vec<u8> = vec![0; 1500];
            let (length, src_addr) = self.socket.recv_from(&mut enc_buf)?;

            if length <= 24 {
                strerror("received buffer too short")?;
            }
            let new_length = length - 24;

            let nonce_raw = enc_buf[new_length..length].to_vec();
            let nonce = XNonce::from_slice(&nonce_raw);
            let key = Key::from_slice(raw_key);
            let cipher = XChaCha20Poly1305::new(key);
            let decrypted = cipher
                .decrypt(nonce, &enc_buf[..new_length])
                .map_err(|e| format!("{:?}", e))?;

            if decrypted.len() % 8 != 0 {
                strerror("decrypted buffer is not octet-aligned")?;
            }
            if decrypted.len() < 24 {
                strerror("decrypted buffer is too short")?;
            }

            let padded = decrypted.len() - 16;

            let crc_gen = Crc::<u64>::new(&crc::CRC_64_ECMA_182);
            let mut digest = crc_gen.digest();
            digest.update(&decrypted[..padded + 8]);
            let crc_result = digest.finalize();

            let mut crc_buf = [0u8; 8];
            crc_buf.copy_from_slice(&decrypted[padded + 8..padded + 16]);
            let crc_received = u64::from_le_bytes(crc_buf);

            if crc_received != crc_result {
                strerror("CRC mismatch")?;
            }

            let mut ts_buf = [0u8; 8];
            ts_buf.copy_from_slice(&decrypted[padded..padded + 8]);
            let ts_received = u64::from_le_bytes(ts_buf);

            let timestamp: u64 = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let dt = if ts_received >= timestamp {
                ts_received - timestamp
            } else {
                timestamp - ts_received
            };
            debug!("UDP TIMESTAMP {}", dt);
            if dt > 10 {
                strerror("time mismatch")?;
            }

            let mut p_buf = [0u8; 2];
            p_buf.copy_from_slice(&decrypted[padded - 2..padded]);
            let p = u16::from_le_bytes(p_buf) as usize;

            buf[..p].copy_from_slice(&decrypted[..p]);

            Ok((p, src_addr))
        } else {
            strerror("No encryption key")?
        }
    }
}
