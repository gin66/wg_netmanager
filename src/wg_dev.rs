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

// wireguard returns an address like this and the %-part has to be removed:[fe80::3bac:744c:f807:a5a2%br-wan]:50001
pub fn v6_strip_interface(sa: &str) -> BoxResult<String> {
    let flds = sa.split('%').collect::<Vec<_>>();
    if flds.len() == 1 {
        Ok(flds[0].to_string())
    }
    else if flds.len() == 2 {
        let rem = flds[1].split(']').collect::<Vec<_>>();
        if rem.len() == 2 {
            Ok(format!("{}]{}",flds[0],rem[1]))
        }
        else {
            Err(format!("invalid address: {}", sa).into())
        }
    }
    else {
        Err(format!("invalid address: {}", sa).into())
    }
}

