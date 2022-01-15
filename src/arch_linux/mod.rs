mod interfaces;
mod wg_dev_linuxkernel;

use std::net::IpAddr;

use crate::wg_dev::*;
use crate::arch_def::Architecture;

use wg_dev_linuxkernel::WireguardDeviceLinux;

pub struct ArchitectureLinux {
}
impl Architecture for ArchitectureLinux {
    fn ipv4v6_socket_setup() -> (bool,bool) {
        // for sysctl net.ipv6.bindv6only=0 systems like linux: ipv6 socket reads/sends ipv4 messages
        (false, true)
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        interfaces::get()
    }
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        Box::new(WireguardDeviceLinux::init(wg_name))
    }
}
