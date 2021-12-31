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
    private_key: Option<String>,
    public_key: Option<String>,
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
    pub fn private_key<T: Into<String>>(mut self, private_key: T) -> Self {
        self.private_key = Some(private_key.into());
        self
    }
    pub fn public_key<T: Into<String>>(mut self, public_key: T) -> Self {
        self.public_key = Some(public_key.into());
        self
    }
    pub fn build(self) -> StaticConfiguration {
        StaticConfiguration {
            verbosity: self.verbosity.unwrap(),
            wg_name: self.wg_name.unwrap(),
            unconnected_ip: self.unconnected_ip.unwrap(),
            new_participant_ip: self.new_participant_ip.unwrap(),
            new_participant_listener_ip: self.new_participant_listener_ip.unwrap(),
            private_key: self.private_key.unwrap(),
            public_key: self.public_key.unwrap(),
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
    pub private_key: String,
    pub public_key: String,
    peers: Vec<PublicPeer>,
}

impl StaticConfiguration {
    pub fn new() -> StaticConfigurationBuilder {
        StaticConfigurationBuilder::new()
    }
    pub fn as_conf(&self) -> String {
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key));
        lines.push(format!("AllowedIPs = {}/32", self.new_participant_listener_ip));
        lines.push(format!("EndPoint = {}", "A"));
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
