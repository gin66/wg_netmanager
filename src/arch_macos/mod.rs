pub mod wg_dev_linuxkernel;

use std::net::IpAddr;

use crate::arch_def::Architecture;
use crate::wg_dev::*;

use wg_dev_linuxkernel::WireguardDeviceLinux;

pub struct ArchitectureMacOs {}
impl Architecture for ArchitectureMacOs {
    fn default_path_to_network_yaml() -> &'static str {
        "network.yaml"
    }
    fn default_path_to_peer_yaml() -> &'static str {
        "peer.yaml"
    }
    fn ipv4v6_socket_setup() -> (bool, bool) {
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
