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
    pub fn wg_name(mut self, wg_name: String) -> Self {
        self.wg_name = Some(wg_name);
        self
    }
    pub fn unconnected_ip(mut self, unconnected_ip: String) -> Self {
        self.unconnected_ip = Some(unconnected_ip);
        self
    }
    pub fn new_participant_ip(mut self, new_participant_ip: String) -> Self {
        self.new_participant_ip = Some(new_participant_ip);
        self
    }
    pub fn new_participant_listener_ip(mut self, new_participant_listener_ip: String) -> Self {
        self.new_participant_listener_ip = Some(new_participant_listener_ip);
        self
    }
    pub fn private_key(mut self, private_key: String) -> Self {
        self.private_key = Some(private_key);
        self
    }
    pub fn public_key(mut self, public_key: String) -> Self {
        self.public_key = Some(public_key);
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
    pub fn new<T: Into<String>>(
        verbosity: Verbosity,
        wg_name: T,
        unconnected_ip: T,
        new_participant_ip: T,
        new_participant_listener_ip: T,
        public_key: T,
        private_key: T,
    ) -> Self {
        StaticConfiguration {
            verbosity,
            wg_name: wg_name.into(),
            unconnected_ip: unconnected_ip.into(),
            new_participant_ip: new_participant_ip.into(),
            new_participant_listener_ip: new_participant_listener_ip.into(),
            public_key: public_key.into(),
            private_key: private_key.into(),
            peers: vec![],
        }
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
