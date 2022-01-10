use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::net::{SocketAddr, UdpSocket};

use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use crc::Crc;
use log::*;
use serde::{Deserialize, Serialize};

use crate::configuration::*;
use crate::error::*;
use crate::manager::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertisementPacket {
    pub public_key: PublicKeyWithTime,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
    pub endpoint: Option<SocketAddr>,
    pub routedb_version: usize,
}
#[derive(Serialize, Deserialize)]
pub struct RouteDatabasePacket {
    pub sender: Ipv4Addr,
    pub routedb_version: usize,
    pub nr_entries: usize,
    pub known_routes: Vec<RouteInfo>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct LocalContactPacket {
    pub public_key: PublicKeyWithTime,
    pub local_ip_list: Vec<IpAddr>,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
}
#[derive(Serialize, Deserialize)]
pub enum UdpPacket {
    Advertisement(AdvertisementPacket),
    RouteDatabaseRequest,
    RouteDatabase(RouteDatabasePacket),
    LocalContactRequest,
    LocalContact(LocalContactPacket),
}
impl UdpPacket {
    pub fn advertisement_from_config(
        static_config: &StaticConfiguration,
        routedb_version: usize,
    ) -> Self {
        let endpoint = if static_config.is_listener() {
            let peer = &static_config.peers[static_config.myself_as_peer.unwrap()];
            Some(SocketAddr::new(peer.public_ip, peer.wg_port))
        } else {
            None
        };
        UdpPacket::Advertisement(AdvertisementPacket {
            public_key: static_config.my_public_key.clone(),
            local_wg_port: static_config.wg_port,
            local_admin_port: static_config.admin_port,
            wg_ip: static_config.wg_ip,
            name: static_config.name.clone(),
            endpoint,
            routedb_version,
        })
    }
    pub fn route_database_request() -> Self {
        UdpPacket::RouteDatabaseRequest {}
    }
    pub fn make_route_database(
        sender: Ipv4Addr,
        routedb_version: usize,
        nr_entries: usize,
        known_routes: Vec<&RouteInfo>,
    ) -> Self {
        UdpPacket::RouteDatabase(RouteDatabasePacket {
            sender,
            routedb_version,
            nr_entries,
            known_routes: known_routes.into_iter().cloned().collect(),
        })
    }
    pub fn local_contact_request() -> Self {
        UdpPacket::LocalContactRequest {}
    }
    pub fn local_contact_from_config(static_config: &StaticConfiguration) -> Self {
        UdpPacket::LocalContact(LocalContactPacket {
            public_key: static_config.my_public_key.clone(),
            local_ip_list: static_config.ip_list.clone(),
            local_wg_port: static_config.wg_port,
            local_admin_port: static_config.admin_port,
            wg_ip: static_config.wg_ip,
            name: static_config.name.clone(),
        })
    }
}
impl fmt::Debug for UdpPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UdpPacket::Advertisement(ad) => ad.fmt(f),
            UdpPacket::RouteDatabaseRequest => f.debug_struct("RouteDatabaseRequest").finish(),
            UdpPacket::RouteDatabase(_) => f.debug_struct("RouteDatabase").finish(),
            UdpPacket::LocalContactRequest => f.debug_struct("LocalContactRequest").finish(),
            UdpPacket::LocalContact(_) => f.debug_struct("LocalContact").finish(),
        }
    }
}

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
    pub fn send_to(&self, payload: &[u8], addr: SocketAddr) -> BoxResult<usize> {
        if let Some(raw_key) = self.key.as_ref() {
            let p = payload.len();
            let padded = ((p + 2 + 7) / 8) * 8; // +2 for 2 Byte length
            let enc_length = padded + 16;

            let timestamp = crate::util::now();
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
            debug!(target: "udp", "send {} Bytes to {:?}", encrypted.len(), addr);
            Ok(self.socket.send_to(&encrypted, addr)?)
        } else {
            strerror("No encryption key")?
        }
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> BoxResult<(usize, SocketAddr)> {
        if let Some(raw_key) = self.key.as_ref() {
            let mut enc_buf: Vec<u8> = vec![0; 1500];
            let (length, src_addr) = self.socket.recv_from(&mut enc_buf)?;
            debug!(target: "udp", "received {} Bytes from {}", length, src_addr);

            if length <= 24 {
                error!(target:"udp", "received buffer too short");
                strerror("received buffer too short")?;
            }
            let new_length = length - 24;

            let nonce_raw = enc_buf[new_length..length].to_vec();
            let nonce = XNonce::from_slice(&nonce_raw);
            let key = Key::from_slice(raw_key);
            let cipher = XChaCha20Poly1305::new(key);
            let decrypted = cipher
                .decrypt(nonce, &enc_buf[..new_length])
                .map_err(|e| format!("Decryption error {:?}", e))?;

            if decrypted.len() % 8 != 0 {
                error!(target:"udp","decrypted buffer is not octet-aligned");
                strerror("decrypted buffer is not octet-aligned")?;
            }
            if decrypted.len() < 24 {
                error!(target:"udp","decrypted buffer is too short");
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
                error!(target:"udp","CRC mismatch");
                strerror("CRC mismatch")?;
            }

            let mut ts_buf = [0u8; 8];
            ts_buf.copy_from_slice(&decrypted[padded..padded + 8]);
            let ts_received = u64::from_le_bytes(ts_buf);

            let timestamp = crate::util::now();
            let dt = if ts_received >= timestamp {
                ts_received - timestamp
            } else {
                timestamp - ts_received
            };
            if dt != 0 {
                debug!("UDP TIMESTAMP {}", dt);
            }
            if dt > 10 {
                error!(target:"udp","time mismatch");
                strerror("time mismatch")?;
            }

            let mut p_buf = [0u8; 2];
            p_buf.copy_from_slice(&decrypted[padded - 2..padded]);
            let p = u16::from_le_bytes(p_buf) as usize;

            buf[..p].copy_from_slice(&decrypted[..p]);

            Ok((p, src_addr))
        } else {
            error!(target:"udp","No encryption key");
            strerror("No encryption key")?
        }
    }
}
