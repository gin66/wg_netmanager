use std::net::IpAddr;

mod interfaces;
pub mod wg_dev_linuxkernel;

pub fn get_local_interfaces() -> Vec<IpAddr> {
    interfaces::get()
}
