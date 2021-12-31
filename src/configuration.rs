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
    wg_name: Option<String>,
    unconnected_ip: Option<String>,
    new_participant_ip: Option<String>,
    new_participant_listener_ip: Option<String>,
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
    pub fn wg_name<T: Into<String>>(mut self, wg_name: T) -> Self {
        self.wg_name = Some(wg_name.into());
        self
    }
    pub fn unconnected_ip<T: Into<String>>(mut self, unconnected_ip: T) -> Self {
        self.unconnected_ip = Some(unconnected_ip.into());
        self
    }
    pub fn new_participant_ip<T: Into<String>>(mut self, new_participant_ip: T) -> Self {
        self.new_participant_ip = Some(new_participant_ip.into());
        self
    }
    pub fn new_participant_listener_ip<T: Into<String>>(mut self, new_participant_listener_ip: T) -> Self {
        self.new_participant_listener_ip = Some(new_participant_listener_ip.into());
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
        StaticConfiguration {
            verbosity: self.verbosity.unwrap(),
            wg_name: self.wg_name.unwrap(),
            unconnected_ip: self.unconnected_ip.unwrap(),
            new_participant_ip: self.new_participant_ip.unwrap(),
            new_participant_listener_ip: self.new_participant_listener_ip.unwrap(),
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
    pub wg_name: String,
    pub unconnected_ip: String,
    pub new_participant_ip: String,
    pub new_participant_listener_ip: String,
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
    pub fn as_conf_for_new_participant(&self, for_peer: usize) -> String {
        let mut lines: Vec<String> = vec![];
        let peer = &self.peers[for_peer];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key_new_participant));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key_listener));
        lines.push(format!("AllowedIPs = {}/32", self.new_participant_listener_ip));
        lines.push(format!("EndPoint = {}:{}", peer.public_ip, peer.join_port));
        lines.push("".to_string());
        lines.join("\n")
    }
    pub fn as_conf_for_listener(&self) -> String {
        let mut lines: Vec<String> = vec![];
        let peer = &self.peers[0];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key_listener));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key_new_participant));
        lines.push(format!("AllowedIPs = {}/32", self.new_participant_ip));
        lines.push("".to_string());
        lines.join("\n")
    }
}

pub enum DynamicConfiguration {
    WithoutDevice,
    Unconfigured,
    ConfiguredForJoin,
    Connected,
    Disconnected,
}
