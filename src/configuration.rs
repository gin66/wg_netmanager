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
    verbosity: Verbosity,
}

impl StaticConfiguration {
    pub fn new(verbosity: Verbosity) -> Self {
        StaticConfiguration { verbosity }
    }
}

pub enum DynamicConfiguration {
    Unconfigured,
    Disconnected,
    Connected,
}
