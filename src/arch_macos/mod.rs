use std::net::IpAddr;

pub mod wg_dev_linuxkernel;

pub fn get_local_interfaces() -> Vec<IpAddr> {
    vec![]
}
