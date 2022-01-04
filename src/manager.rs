
//
// Any peer will send out their view of the network on request.
//
// One possibility:
//      Versioned list of (wg_ip, connnected_to_wg_ip, timestamp)
//
// On every change the version is updated and status info sent out to the direct peers
// If a peer does not know this, then it will request an updated list
// 
// The NetworkManager shall provide as output
//      All info to set up routing to the network nodes with gateway information
//      wg_ip list of peers in order to request the public key and endpoints
//      proposal for trying short routes
//
// As input:
//      list of dynamic peers
//      answers from other nodes
//      status info from other nodes
//
// For testing:
//      allow multiple instances of NetworkManager, which can be connected by glue code freely
//

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use crate::configuration::*;

pub struct NodeInfo {
    timestamp: u64,
    public_key: Option<PublicKeyWithTime>,
}

pub struct RouteInfo {
    to: Ipv4Addr,
    gateway: Option<Ipv4Addr>,
}

#[derive(Default)]
pub struct RouteDB {
    version: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    all_nodes: HashMap<Ipv4Addr, NodeInfo>,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, RouteDB>,
}

impl NetworkManager {
    pub fn new(wg_ip: Ipv4Addr) -> Self {
        NetworkManager {
            wg_ip: wg_ip,
            all_nodes: HashMap::new(),
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),
        }
    }

    pub fn add_dynamic_peer(&mut self, peer: &DynamicPeer) {
    }
    pub fn remove_dynamic_peer(&mut self, peer: &DynamicPeer) {
    }


    pub fn get_routes(&mut self) -> Vec<&RouteInfo> {
        vec![]
    }
}
