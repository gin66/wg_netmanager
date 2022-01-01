use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub enum Verbosity {
    Silent,
    Info,
    All,
}
impl Verbosity {
    pub fn info(&self) -> bool {
        match self {
            Verbosity::Info | Verbosity::All => true,
            _ => false,
        }
    }
    pub fn all(&self) -> bool {
        match self {
            Verbosity::All => true,
            _ => false,
        }
    }
}

pub struct PublicPeer {
    pub public_ip: String,
    pub join_port: u16,
    pub comm_port: u16,
    pub udp_port: u16,
    pub wg_ip: String,
}

#[derive(Default)]
pub struct StaticConfigurationBuilder {
    verbosity: Option<Verbosity>,
    name: Option<String>,
    wg_ip: Option<String>,
    wg_name: Option<String>,
    new_participant_ip: Option<String>,
    new_participant_listener_ip: Option<String>,
    my_private_key: Option<String>,
    my_public_key: Option<String>,
    private_key_listener: Option<String>,
    public_key_listener: Option<String>,
    private_key_new_participant: Option<String>,
    public_key_new_participant: Option<String>,
    peers: Vec<PublicPeer>,
}
impl StaticConfigurationBuilder {
    pub fn new() -> Self {
        StaticConfigurationBuilder::default()
    }
    pub fn verbosity(mut self, verbosity: Verbosity) -> Self {
        self.verbosity = Some(verbosity);
        self
    }
    pub fn name<T: Into<String>>(mut self, name: T) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn wg_ip<T: Into<String>>(mut self, wg_ip: T) -> Self {
        self.wg_ip = Some(wg_ip.into());
        self
    }
    pub fn wg_name<T: Into<String>>(mut self, wg_name: T) -> Self {
        self.wg_name = Some(wg_name.into());
        self
    }
    pub fn new_participant_ip<T: Into<String>>(mut self, new_participant_ip: T) -> Self {
        self.new_participant_ip = Some(new_participant_ip.into());
        self
    }
    pub fn new_participant_listener_ip<T: Into<String>>(
        mut self,
        new_participant_listener_ip: T,
    ) -> Self {
        self.new_participant_listener_ip = Some(new_participant_listener_ip.into());
        self
    }
    pub fn my_private_key<T: Into<String>>(mut self, private_key: T) -> Self {
        self.my_private_key = Some(private_key.into());
        self
    }
    pub fn my_public_key<T: Into<String>>(mut self, public_key: T) -> Self {
        self.my_public_key = Some(public_key.into());
        self
    }
    pub fn private_key_listener<T: Into<String>>(mut self, private_key: T) -> Self {
        self.private_key_listener = Some(private_key.into());
        self
    }
    pub fn public_key_listener<T: Into<String>>(mut self, public_key: T) -> Self {
        self.public_key_listener = Some(public_key.into());
        self
    }
    pub fn private_key_new_participant<T: Into<String>>(mut self, private_key: T) -> Self {
        self.private_key_new_participant = Some(private_key.into());
        self
    }
    pub fn public_key_new_participant<T: Into<String>>(mut self, public_key: T) -> Self {
        self.public_key_new_participant = Some(public_key.into());
        self
    }
    pub fn peers(mut self, peers: Vec<PublicPeer>) -> Self {
        self.peers = peers;
        self
    }
    pub fn build(self) -> StaticConfiguration {
        let mut myself_as_peer: Option<usize> = None;
        for (i, peer) in self.peers.iter().enumerate() {
            if &peer.wg_ip == self.wg_ip.as_ref().unwrap() {
                println!("FOUND");
                myself_as_peer = Some(i);
                break;
            }
        }

        let peer_cnt = self.peers.len();
        StaticConfiguration {
            verbosity: self.verbosity.unwrap(),
            name: self.name.unwrap(),
            wg_ip: self.wg_ip.unwrap(),
            wg_name: self.wg_name.unwrap(),
            myself_as_peer,
            new_participant_ip: self.new_participant_ip.unwrap(),
            new_participant_listener_ip: self.new_participant_listener_ip.unwrap(),
            my_private_key: self.my_private_key.unwrap(),
            my_public_key: self.my_public_key.unwrap(),
            private_key_listener: self.private_key_listener.unwrap(),
            public_key_listener: self.public_key_listener.unwrap(),
            private_key_new_participant: self.private_key_new_participant.unwrap(),
            public_key_new_participant: self.public_key_new_participant.unwrap(),
            peers: self.peers,
            peer_cnt,
        }
    }
}

pub struct StaticConfiguration {
    pub verbosity: Verbosity,
    pub name: String,
    pub wg_ip: String,
    pub wg_name: String,
    myself_as_peer: Option<usize>,
    pub new_participant_ip: String,
    pub new_participant_listener_ip: String,
    pub my_private_key: String,
    pub my_public_key: String,
    pub private_key_listener: String,
    pub public_key_listener: String,
    pub private_key_new_participant: String,
    pub public_key_new_participant: String,
    peers: Vec<PublicPeer>,
    pub peer_cnt: usize,
}

impl StaticConfiguration {
    pub fn new() -> StaticConfigurationBuilder {
        StaticConfigurationBuilder::new()
    }
    pub fn is_listener(&self) -> bool {
        self.myself_as_peer.is_some()
    }
    pub fn as_conf_for_new_participant(&self, for_peer: usize) -> String {
        let mut lines: Vec<String> = vec![];
        let peer = &self.peers[for_peer];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key_new_participant));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key_listener));
        lines.push(format!(
            "AllowedIPs = {}/32",
            self.new_participant_listener_ip
        ));
        lines.push(format!("EndPoint = {}:{}", peer.public_ip, peer.join_port));
        lines.push("".to_string());
        lines.join("\n")
    }
    pub fn as_conf_for_listener(&self) -> String {
        let peer = &self.peers[self.myself_as_peer.unwrap()];
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key_listener));
        lines.push(format!("ListenPort = {}", peer.join_port));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key_new_participant));
        lines.push(format!("AllowedIPs = {}/32", self.new_participant_ip));
        lines.push("".to_string());
        lines.join("\n")
    }
    pub fn as_conf_as_peer(&self, dynamic_peers: Option<&DynamicPeerList>) -> String {
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.my_private_key));
        if let Some(myself) = self.myself_as_peer {
            let peer = &self.peers[myself];
            lines.push(format!("ListenPort = {}", peer.comm_port));
        }
        lines.push("".to_string());

        if let Some(peers) = dynamic_peers {
            for peer in peers.peer.values() {
                lines.push("[Peer]".to_string());
                lines.push(format!("PublicKey = {}", peer.public_key));
                lines.push(format!("AllowedIPs = {}/32", peer.wg_ip));
                if let Some(endpoint) = peer.endpoint.as_ref() {
                    lines.push(format!("EndPoint = {}", endpoint));
                }
                lines.push("".to_string());
            }
        }

        lines.join("\n")
    }
    pub fn my_udp_port(&self) -> Option<u16> {
        self.myself_as_peer.map(|i| self.peers[i].udp_port)
    }
    pub fn udp_port(&self, peer_index: usize) -> u16 {
        self.peers[peer_index].udp_port
    }
}

#[derive(Debug)]
pub struct DynamicPeer {
    public_key: String,
    wg_ip: String,
    name: String,
    endpoint: Option<String>,
    comm_port: u16,
    lastseen: Instant,
}

#[derive(Default)]
pub struct DynamicPeerList {
    pub peer: HashMap<String, DynamicPeer>,
    pub fifo_dead: Vec<String>,
    pub fifo_ping: Vec<String>,
}
impl DynamicPeerList {
    pub fn add_peer(&mut self, from_advertisement: UdpPacket, comm_port: u16) -> Option<String> {
        use UdpPacket::*;
        match from_advertisement {
            ListenerAdvertisement {
                public_key,
                wg_ip,
                name,
                endpoint,
            } => {
                self.fifo_dead.push(wg_ip.clone());
                self.fifo_ping.push(wg_ip.clone());
                let lastseen = Instant::now();
                let key = wg_ip.clone();
                let new_wg_ip = wg_ip.clone();
                if self
                    .peer
                    .insert(
                        key,
                        DynamicPeer {
                            wg_ip,
                            public_key,
                            name,
                            endpoint: Some(endpoint),
                            comm_port,
                            lastseen,
                        },
                    )
                    .is_none()
                {
                    Some(new_wg_ip)
                } else {
                    None
                }
            }
            ClientAdvertisement {
                public_key,
                wg_ip,
                name,
            } => {
                self.fifo_dead.push(wg_ip.clone());
                self.fifo_ping.push(wg_ip.clone());
                let lastseen = Instant::now();
                let key = wg_ip.clone();
                let new_wg_ip = wg_ip.clone();
                if self
                    .peer
                    .insert(
                        key,
                        DynamicPeer {
                            wg_ip,
                            public_key,
                            name,
                            endpoint: None,
                            comm_port,
                            lastseen,
                        },
                    )
                    .is_none()
                {
                    Some(new_wg_ip)
                } else {
                    None
                }
            }
        }
    }
    pub fn check_timeouts(&mut self) -> Vec<String> {
        let mut dead_peers = vec![];
        while let Some(wg_ip) = self.fifo_dead.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                if peer.lastseen.elapsed().as_secs() < 60 {
                    break;
                }
                dead_peers.push(wg_ip.to_string());
            }
            self.fifo_dead.remove(0);
        }
        dead_peers
    }
    pub fn check_ping_timeouts(&mut self) -> Vec<(String, u16)> {
        let mut ping_peers = vec![];
        while let Some(wg_ip) = self.fifo_ping.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                if peer.lastseen.elapsed().as_secs() < 30 {
                    break;
                }
                ping_peers.push((wg_ip.to_string(), 55555 /*peer.comm_port*/));
            }
            self.fifo_ping.remove(0);
        }
        ping_peers
    }
    pub fn remove_peer(&mut self, wg_ip: &str) {
        self.peer.remove(wg_ip);
    }
    pub fn output(&self) {
        println!("Dynamic Peers:");
        for peer in self.peer.values() {
            println!("{:?}", peer);
        }
        println!("");
    }
}

pub enum DynamicConfigurationClient {
    WithoutDevice,
    Unconfigured { peer_index: usize },
    ConfiguredForJoin { peer_index: usize },
    WaitForAdvertisement { peer_index: usize, cnt: u8 },
    Connected,
}

#[derive(Serialize, Deserialize)]
pub enum UdpPacket {
    // TODO: Change from String to &str
    ListenerAdvertisement {
        public_key: String,
        wg_ip: String,
        name: String,
        endpoint: String,
    },
    ClientAdvertisement {
        public_key: String,
        wg_ip: String,
        name: String,
    },
}
impl UdpPacket {
    pub fn advertisement_from_config(static_config: &StaticConfiguration) -> Self {
        if static_config.is_listener() {
            let peer = &static_config.peers[static_config.myself_as_peer.unwrap()];
            UdpPacket::ListenerAdvertisement {
                public_key: static_config.my_public_key.clone(),
                wg_ip: static_config.wg_ip.clone(),
                name: static_config.name.clone(),
                endpoint: format!("{}:{}", peer.public_ip, peer.comm_port),
            }
        } else {
            UdpPacket::ClientAdvertisement {
                public_key: static_config.my_public_key.clone(),
                wg_ip: static_config.wg_ip.clone(),
                name: static_config.name.clone(),
            }
        }
    }
}
