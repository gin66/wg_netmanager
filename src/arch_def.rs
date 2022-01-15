use std::net::IpAddr;

use crate::wg_dev::WireguardDevice;

pub trait Architecture {
    fn ipv4v6_socket_setup() -> (bool,bool);
    fn get_local_interfaces() -> Vec<IpAddr>;
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice>;
}

