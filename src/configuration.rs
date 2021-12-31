use std::net::UdpSocket;
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
        self.my_private_key= Some(private_key.into());
        self
    }
    pub fn my_public_key<T: Into<String>>(mut self, public_key: T) -> Self {
        self.my_public_key= Some(public_key.into());
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
    pub fn as_conf_as_peer(&self) -> String {
        let peer = &self.peers[self.myself_as_peer.unwrap()];
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.my_private_key));
        lines.push(format!("ListenPort = {}", peer.comm_port));
        lines.push("".to_string());
        lines.join("\n")
    }
}

pub enum DynamicConfigurationListener {
    WithoutDevice,
    Unconfigured,
    ConfiguredForJoin {
        socket: UdpSocket,
    },
    Connected,
    Disconnected,
}
pub enum DynamicConfigurationClient {
    WithoutDevice,
    Unconfigured,
    ConfiguredForJoin {
        socket: UdpSocket,
    },
    WaitForAdvertisement {
        socket: UdpSocket,
        cnt: u8,
    },
    Connected,
    Disconnected,
}

#[derive(Serialize, Deserialize)]
pub enum UdpAdvertisement {
    // TODO: Change from String to &str
    ListenerAdvertisement {
        public_key: String,
        wg_ip: String,
        name: String,
    },
    ClientAdvertisement {
        public_key: String,
        wg_ip: String,
        name: String,
    }
}
impl UdpAdvertisement {
    pub fn from_config(static_config: &StaticConfiguration) -> Self {
        if static_config.is_listener() {
            UdpAdvertisement::ListenerAdvertisement {
                public_key: static_config.my_public_key.clone(),
                wg_ip: static_config.wg_ip.clone(),
                name: static_config.name.clone(),
            }
        }
        else {
            UdpAdvertisement::ClientAdvertisement {
                public_key: static_config.my_public_key.clone(),
                wg_ip: static_config.wg_ip.clone(),
                name: static_config.name.clone(),
            }
        }
    }
}
