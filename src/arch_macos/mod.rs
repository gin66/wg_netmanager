pub mod wg_dev_macos;

use std::net::IpAddr;

use crate::arch_def::Architecture;
use crate::wg_dev::*;

use wg_dev_macos::WireguardDeviceMacos;

pub struct ArchitectureMacOs {}
impl Architecture for ArchitectureMacOs {
    fn default_path_to_network_yaml() -> &'static str {
        "network.yaml"
    }
    fn default_path_to_peer_yaml() -> &'static str {
        "peer.yaml"
    }
    fn ipv4v6_socket_setup() -> (bool, bool, bool) {
        (true, true, true)
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        vec![]
    }
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        Box::new(WireguardDeviceMacos::init(wg_name))
    }
}
