use std::net::Ipv4Addr;

use crate::error::*;
pub use crate::wg_dev_linuxkernel::*;

pub trait WireguardDevice {
    fn check_device(&self) -> BoxResult<bool>;
    fn bring_up_device(&self) -> BoxResult<()>;
    fn take_down_device(&self) -> BoxResult<()>;
    fn set_ip(&self, ip: &Ipv4Addr) -> BoxResult<()>;
    fn add_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn del_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn set_conf(&self, conf: &str) -> BoxResult<()>;
    fn sync_conf(&self, conf: &str) -> BoxResult<()>;
    fn flush_all(&self) -> BoxResult<()>;
}
