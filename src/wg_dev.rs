pub use crate::wg_dev_linuxkernel::*;
use std::net::Ipv4Addr;

pub trait WireguardDevice {
    fn check_device(&self) -> std::io::Result<bool>;
    fn bring_up_device(&self) -> std::io::Result<()>;
    fn take_down_device(&self) -> std::io::Result<()>;
    fn set_ip(&self, ip: &Ipv4Addr) -> std::io::Result<()>;
    fn add_route(&self, route: &str) -> std::io::Result<()>;
    fn del_route(&self, route: &str) -> std::io::Result<()>;
    fn set_conf(&self, conf: &str) -> Result<(), String>;
    fn sync_conf(&self, conf: &str) -> Result<(), String>;
}
