use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use ipnet::Ipv4Net;

use crate::error::*;

pub trait WireguardDevice {
    fn check_device(&self) -> BoxResult<bool>;
    fn create_device(&self) -> BoxResult<()>;
    fn take_down_device(&self) -> BoxResult<()>;
    fn set_ip(&mut self, ip: &Ipv4Addr, subnet: &Ipv4Net) -> BoxResult<()>;
    fn add_route(&self, host: Ipv4Addr, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn replace_route(&self, host: Ipv4Addr, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn del_route(&self, host: Ipv4Addr, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn set_conf(&self, conf: &str) -> BoxResult<()>;
    fn sync_conf(&self, conf: &str) -> BoxResult<()>;
    fn flush_all(&self) -> BoxResult<()>;
    fn retrieve_conf(&self) -> BoxResult<HashMap<String, SocketAddr>>;
    fn create_key_pair(&self) -> BoxResult<(String, String)>;
}

pub fn map_to_ipv6(ipv4: &Ipv4Addr) -> Ipv6Addr {
    let mut segments = ipv4.to_ipv6_mapped().segments();
    segments[0] = 0xfd00;
    Ipv6Addr::from(segments)
}
