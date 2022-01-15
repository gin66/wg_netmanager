use std::net::IpAddr;

use crate::arch_def::Architecture;
use crate::wg_dev::*;

pub struct ArchitectureAndroid {}
impl Architecture for ArchitectureAndroid {
    fn ipv4v6_socket_setup() -> (bool, bool) {
        unimplemented!();
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        vec![]
    }
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        unimplemented!();
    }
}
