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

pub struct StaticConfiguration {
    pub verbosity: Verbosity,
    pub wg_name: String,
    pub unconnected_ip: String,
    pub private_key: String,
    pub public_key: String,
}

impl StaticConfiguration {
    pub fn new<T: Into<String>>(
        verbosity: Verbosity,
        wg_name: T,
        unconnected_ip: T,
        private_key: T,
    ) -> Self {
        StaticConfiguration {
            verbosity,
            wg_name: wg_name.into(),
            unconnected_ip: unconnected_ip.into(),
            private_key: private_key.into(),
            public_key: private_key.into(),
        }
    }
    pub fn as_conf(&self) -> String {
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.private_key));
        lines.push("".to_string());
        lines.push("[Peer]".to_string());
        lines.push(format!("PublicKey = {}", self.public_key));
        lines.push(format!("AllowedIPs = {}", "A"));
        lines.push(format!("EndPoint = {}", "A"));
        lines.push("".to_string());
        lines.join("\n")
    }
}

pub enum DynamicConfiguration {
    WithoutDevice,
    Unconfigured,
    Disconnected,
    Connected,
}
