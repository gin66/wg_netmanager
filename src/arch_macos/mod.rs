pub mod wg_dev_linuxkernel;

use std::net::IpAddr;

use crate::wg_dev::*;
use crate::arch_def::Architecture;

pub struct ArchitectureMacOs {
}
impl Architecture for ArchitectureLinux {
    fn ipv4v6_socket_setup() -> (bool,bool) {
        // compromise on macos not being able to do NAT traversal
        (true, false)
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        vec![]
    }
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        Box::new(WireguardDeviceLinux::init(wg_name))
    }
}
