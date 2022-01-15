use std::net::IpAddr;
use std::sync::mpsc;

use clap::ArgMatches;

use crate::configuration::StaticConfiguration;
use crate::error::BoxResult;
use crate::event::Event;
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
    fn arch_specific_init(tx: mpsc::Sender<Event>) {}
    #[allow(unused_variables)]
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        unimplemented!();
    }
    #[allow(unused_variables)]
    fn command_install(matches: &ArgMatches, static_config: StaticConfiguration) -> BoxResult<()> {
        unimplemented!();
    }
}
