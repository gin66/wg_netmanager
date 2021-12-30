#[derive(Clone,Copy)]
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
}

impl StaticConfiguration {
    pub fn new<T: Into<String>>(verbosity: Verbosity, wg_name: T, unconnected_ip: T) -> Self {
        StaticConfiguration { verbosity, wg_name: wg_name.into(), unconnected_ip: unconnected_ip.into() }
    }
}

pub enum DynamicConfiguration {
    WithoutDevice,
    Unconfigured,
    Disconnected,
    Connected,
}

