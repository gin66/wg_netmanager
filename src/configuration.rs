use std::net::{IpAddr, Ipv4Addr};

use log::*;
use serde::{Deserialize, Serialize};

use crate::manager::*;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PublicKeyWithTime {
    pub key: String, // base64 encoded
    pub priv_key_creation_time: u64,
}

pub struct PublicPeer {
    pub public_ip: IpAddr,
    pub wg_port: u16,
    pub admin_port: u16,
    pub wg_ip: Ipv4Addr,
}

#[derive(Default)]
pub struct StaticConfigurationBuilder {
    name: Option<String>,
    ip_list: Option<Vec<IpAddr>>,
    wg_ip: Option<Ipv4Addr>,
    wg_name: Option<String>,
    wg_port: Option<u16>,
    admin_port: Option<u16>,
    shared_key: Option<Vec<u8>>,
    my_private_key: Option<String>,
    my_public_key: Option<PublicKeyWithTime>,
    peers: Vec<PublicPeer>,
    use_tui: Option<bool>,
}
impl StaticConfigurationBuilder {
    pub fn new() -> Self {
        StaticConfigurationBuilder::default()
    }
    pub fn name<T: Into<String>>(mut self, name: T) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn ip_list(mut self, ip_list: Vec<IpAddr>) -> Self {
        self.ip_list = Some(ip_list);
        self
    }
    pub fn wg_ip<T: Into<Ipv4Addr>>(mut self, wg_ip: T) -> Self {
        self.wg_ip = Some(wg_ip.into());
        self
    }
    pub fn wg_name<T: Into<String>>(mut self, wg_name: T) -> Self {
        self.wg_name = Some(wg_name.into());
        self
    }
    pub fn wg_port(mut self, port: u16) -> Self {
        self.wg_port = Some(port);
        self
    }
    pub fn admin_port(mut self, port: u16) -> Self {
        self.admin_port = Some(port);
        self
    }
    pub fn shared_key(mut self, shared_key: Vec<u8>) -> Self {
        self.shared_key = Some(shared_key);
        self
    }
    pub fn my_private_key<T: Into<String>>(mut self, private_key: T) -> Self {
        self.my_private_key = Some(private_key.into());
        self
    }
    pub fn my_public_key(mut self, public_key: PublicKeyWithTime) -> Self {
        self.my_public_key = Some(public_key);
        self
    }
    pub fn peers(mut self, peers: Vec<PublicPeer>) -> Self {
        self.peers = peers;
        self
    }
    pub fn use_tui(mut self, use_tui: bool) -> Self {
        self.use_tui = Some(use_tui);
        self
    }
    pub fn build(self) -> StaticConfiguration {
        let mut myself_as_peer: Option<usize> = None;
        for (i, peer) in self.peers.iter().enumerate() {
            if &peer.wg_ip == self.wg_ip.as_ref().unwrap() {
                debug!("FOUND myself as listener");
                myself_as_peer = Some(i);
                break;
            }
        }

        let peer_cnt = self.peers.len();
        StaticConfiguration {
            name: self.name.unwrap(),
            ip_list: self.ip_list.unwrap(),
            wg_ip: self.wg_ip.unwrap(),
            wg_name: self.wg_name.unwrap(),
            wg_port: self.wg_port.unwrap(),
            admin_port: self.admin_port.unwrap(),
            myself_as_peer,
            shared_key: self.shared_key.unwrap(),
            my_private_key: self.my_private_key.unwrap(),
            my_public_key: self.my_public_key.unwrap(),
            peers: self.peers,
            peer_cnt,
            use_tui: self.use_tui.unwrap(),
        }
    }
}

pub struct StaticConfiguration {
    pub name: String,
    pub ip_list: Vec<IpAddr>,
    pub wg_ip: Ipv4Addr,
    pub wg_name: String,
    pub wg_port: u16,
    pub admin_port: u16,
    pub myself_as_peer: Option<usize>,
    pub shared_key: Vec<u8>,
    pub my_private_key: String,
    pub my_public_key: PublicKeyWithTime,
    pub peers: Vec<PublicPeer>,
    pub peer_cnt: usize,
    pub use_tui: bool,
}

impl StaticConfiguration {
    pub fn builder() -> StaticConfigurationBuilder {
        StaticConfigurationBuilder::new()
    }
    pub fn is_listener(&self) -> bool {
        self.myself_as_peer.is_some()
    }
    pub fn as_conf_as_peer(&self, manager: &NetworkManager) -> String {
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.my_private_key));
        let port = if let Some(myself) = self.myself_as_peer {
            let peer = &self.peers[myself];
            peer.wg_port
        } else {
            self.wg_port
        };
        lines.push(format!("ListenPort = {}", port));
        lines.push("".to_string());

        for peer in manager.peer.values() {
            lines.push("[Peer]".to_string());
            lines.push(format!("PublicKey = {}", &peer.public_key.key));
            lines.push(format!("AllowedIPs = {}/32", peer.wg_ip));
            let ips = manager.get_ips_for_peer(peer.wg_ip);
            for ip in ips {
                lines.push(format!("AllowedIPs = {}/32", ip));
            }
            if let Some(endpoint) = peer.endpoint.as_ref() {
                lines.push(format!("EndPoint = {}", endpoint));
            }
            lines.push("".to_string());
        }

        lines.join("\n")
    }
    pub fn my_admin_port(&self) -> u16 {
        self.myself_as_peer
            .map(|i| self.peers[i].admin_port)
            .unwrap_or(self.admin_port)
    }
    pub fn admin_port(&self, peer_index: usize) -> u16 {
        self.peers[peer_index].admin_port
    }
}
