use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use ipnet::Ipv4Net;

use crate::error::*;
use crate::arch::*;

pub trait WireguardDevice {
    fn check_device(&self) -> BoxResult<bool>;
    fn bring_up_device(&self) -> BoxResult<()>;
    fn take_down_device(&self) -> BoxResult<()>;
    fn set_ip(&mut self, ip: &Ipv4Addr, subnet: &Ipv4Net) -> BoxResult<()>;
    fn add_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn replace_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn del_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()>;
    fn set_conf(&self, conf: &str) -> BoxResult<()>;
    fn sync_conf(&self, conf: &str) -> BoxResult<()>;
    fn flush_all(&self) -> BoxResult<()>;
    fn retrieve_conf(&self) -> BoxResult<HashMap<String, SocketAddr>>;
    fn create_key_pair(&self) -> BoxResult<(String, String)>;
}

#[cfg(target_os = "linux")]
pub fn get_wireguard_device_linux<T: Into<String>>(
    wg_name: T,
) -> BoxResult<Box<dyn WireguardDevice>> {
    // here is the place to detect capabilities of the environment

    Ok(Box::new(ArchWireguardDevice::init(wg_name)))
}

#[cfg(target_os = "macos")]
pub fn get_wireguard_device_linux<T: Into<String>>(
    wg_name: T,
) -> BoxResult<Box<dyn WireguardDevice>> {
    // here is the place to detect capabilities of the environment

    Ok(Box::new(WireguardDeviceLinux::init(wg_name)))
}

pub fn get_wireguard_device<T: Into<String>>(wg_name: T) -> BoxResult<Box<dyn WireguardDevice>> {
    // os dependent initialization

    #[cfg(target_os = "linux")]
    return get_wireguard_device_linux(wg_name);

    #[cfg(target_os = "macos")]
    return get_wireguard_device_linux(wg_name);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err("Unsupported OS".into())
}

pub fn map_to_ipv6(ipv4: &Ipv4Addr) -> Ipv6Addr {
    let mut segments = ipv4.to_ipv6_mapped().segments();
    segments[0] = 0xfd00;
    Ipv6Addr::from(segments)
}
