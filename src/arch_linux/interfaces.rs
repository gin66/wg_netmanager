use std::net::IpAddr;

use log::*;

pub fn get() -> Vec<IpAddr> {
    let ifaces = ifcfg::IfCfg::get().expect("could not get interfaces");
    let mut ip_list: Vec<IpAddr> = vec![];
    trace!("Interfaces");
    for iface in ifaces.iter() {
        for addr in iface.addresses.iter() {
            use ifcfg::AddressFamily::*;
            match addr.address_family {
                IPv4 => {
                    trace!("{:#?}", addr.address.as_ref().unwrap().ip());
                    ip_list.push(addr.address.as_ref().unwrap().ip());
                }
                IPv6 => {
                    trace!("{:#?}", addr.address.as_ref().unwrap().ip());
                    ip_list.push(addr.address.as_ref().unwrap().ip());
                }
                _ => {}
            }
        }
    }
    let ip_list = ip_list.into_iter().filter(|ip| !ip.is_loopback()).collect();
    debug!("Interfaces: {:#?}", ip_list);
    ip_list
}
