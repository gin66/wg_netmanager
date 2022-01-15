use std::net::IpAddr;

use crate::wg_dev::WireguardDevice;

pub trait Architecture {
    fn default_path_to_network_yaml() -> &'static str {
        "network.yaml"
    }
    fn default_path_to_peer_yaml() -> &'static str {
        "peer.yaml"
    }
    fn default_wireguard_interface() -> &'static str {
        "undefined"
    }
    fn ipv4v6_socket_setup() -> (bool, bool) {
        unimplemented!();
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        vec![]
    }
    #[allow(unused_variables)]
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        unimplemented!();
    }
}
