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

#[derive(Debug)]
pub enum RouteChange {
    AddRouteWithGateway { to: Ipv4Addr, gateway: Ipv4Addr },
    AddRoute { to: Ipv4Addr },
    DelRouteWithGateway { to: Ipv4Addr, gateway: Ipv4Addr },
    DelRoute { to: Ipv4Addr },
}

pub struct NodeInfo {
    timestamp: u64,
    public_key: Option<PublicKeyWithTime>,
}

struct RouteInfo {
    to: Ipv4Addr,
    gateway: Option<Ipv4Addr>,
    issued: bool,
    to_be_deleted: bool,
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

    pub fn add_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        // Dynamic peers are ALWAYS reachable without a gateway
        let ri = RouteInfo {
            to: *peer_ip,
            gateway: None,
            issued: false,
            to_be_deleted: false,
        };
        self.route_db.route_for.insert(*peer_ip, ri);
        self.route_db.version += 1;
    }
    pub fn remove_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        if let Some(ref mut ri) = self.route_db.route_for.get_mut(peer_ip) {
            ri.to_be_deleted = true;
        } else {
            panic!("should not happe");
        }
    }

    pub fn get_route_changes(&mut self) -> Vec<RouteChange> {
        let mut routes = vec![];

        // first routes to be deleted
        for ri in self
            .route_db
            .route_for
            .values_mut()
            .filter(|ri| ri.to_be_deleted && ri.issued)
        {
            routes.push(RouteChange::DelRoute { to: ri.to });
            ri.issued = false;
        }

        self.route_db.route_for.retain(|_, ri| !ri.to_be_deleted);

        // then routes to be added
        for ri in self.route_db.route_for.values_mut().filter(|ri| !ri.issued) {
            ri.issued = true;
            routes.push(RouteChange::AddRoute { to: ri.to });
        }

        routes
    }
}
