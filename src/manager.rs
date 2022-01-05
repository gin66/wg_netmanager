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

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::Ipv4Addr;
//use std::net::SocketAddr;

use log::*;
use serde::{Deserialize, Serialize};

use crate::configuration::*;

#[derive(Debug)]
pub enum RouteChange {
    AddRouteWithGateway { to: Ipv4Addr, gateway: Ipv4Addr },
    AddRoute { to: Ipv4Addr },
    DelRouteWithGateway { to: Ipv4Addr, gateway: Ipv4Addr },
    DelRoute { to: Ipv4Addr },
}

//pub struct NodeInfo {
//    timestamp: u64,
//    public_key: Option<PublicKeyWithTime>,
//}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Gateway {
    hop_cnt: usize,
    ip: Ipv4Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteInfo {
    to: Ipv4Addr,
    gateway: Option<Gateway>,
}

struct RouteInfoWithStatus {
    ri: RouteInfo,
    issued: bool,
    to_be_deleted: bool,
}

#[derive(Default)]
pub struct RouteDB {
    version: usize,
    route_for: HashMap<Ipv4Addr, RouteInfoWithStatus>,
}

#[derive(Default)]
pub struct PeerRouteDB {
    version: usize,
    nr_entries: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    //all_nodes: HashMap<Ipv4Addr, NodeInfo>,
    peers: HashSet<Ipv4Addr>,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, PeerRouteDB>,
}

impl NetworkManager {
    pub fn new(wg_ip: Ipv4Addr) -> Self {
        NetworkManager {
            wg_ip,
            //all_nodes: HashMap::new(),
            peers: HashSet::new(),
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),
        }
    }

    pub fn add_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        self.peers.insert(*peer_ip);
        self.recalculate_routes();

        // Dynamic peers are ALWAYS reachable without a gateway
        let ri = RouteInfoWithStatus {
            ri: RouteInfo {
                to: *peer_ip,
                gateway: None,
            },
            issued: false,
            to_be_deleted: false,
        };
        self.route_db.route_for.insert(*peer_ip, ri);
    }
    pub fn remove_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        self.peers.remove(peer_ip);
        self.recalculate_routes();

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
            ri.issued = false;
            if let Some(gateway) = ri.ri.gateway.as_ref() {
                routes.push(RouteChange::DelRouteWithGateway {
                    to: ri.ri.to,
                    gateway: gateway.ip,
                });
            } else {
                routes.push(RouteChange::DelRoute { to: ri.ri.to });
            }
        }

        self.route_db.route_for.retain(|_, ri| !ri.to_be_deleted);

        // then routes to be added
        for ri in self.route_db.route_for.values_mut().filter(|ri| !ri.issued) {
            ri.issued = true;
            if let Some(gateway) = ri.ri.gateway.as_ref() {
                routes.push(RouteChange::AddRouteWithGateway {
                    to: ri.ri.to,
                    gateway: gateway.ip,
                });
            } else {
                routes.push(RouteChange::AddRoute { to: ri.ri.to });
            }
        }

        routes
    }
    pub fn db_version(&self) -> usize {
        self.route_db.version
    }
    pub fn analyze_advertisement(&mut self, udp_packet: &UdpPacket) -> Option<Ipv4Addr> {
        use UdpPacket::*;
        match udp_packet {
            RouteDatabaseRequest { .. } => None,
            RouteDatabase { .. } => None,
            Advertisement {
                wg_ip,
                routedb_version,
                ..
            } => {
                if let Some(peer_route_db) = self.peer_route_db.get(wg_ip) {
                    if peer_route_db.version == *routedb_version {
                        return None;
                    }
                    self.peer_route_db.remove(wg_ip);
                }
                Some(*wg_ip)
            }
        }
    }
    pub fn provide_route_database(&self) -> Vec<UdpPacket> {
        let mut known_routes = vec![];
        for ri in self.route_db.route_for.values().filter(|ri| ri.issued) {
            known_routes.push(&ri.ri);
        }
        let p = UdpPacket::make_route_database(
            self.wg_ip,
            self.route_db.version,
            known_routes.len(),
            known_routes,
        );
        vec![p]
    }
    pub fn process_route_database(&mut self, udp_packet: UdpPacket) -> bool {
        use UdpPacket::*;
        let mut need_routes_update = false;
        match udp_packet {
            Advertisement { .. } => {}
            RouteDatabaseRequest { .. } => {}
            RouteDatabase {
                sender,
                known_routes,
                routedb_version,
                nr_entries,
            } => {
                debug!("RouteDatabase from {}: {:?}", sender, known_routes);
                if let Some(mut peer_route_db) = self.peer_route_db.remove(&sender) {
                    if nr_entries == peer_route_db.nr_entries {
                        if routedb_version == peer_route_db.version {
                            for ri in known_routes {
                                peer_route_db.route_for.insert(ri.to, ri);
                            }
                            need_routes_update = nr_entries == peer_route_db.route_for.len();
                            self.peer_route_db.insert(sender, peer_route_db);
                        } else {
                            warn!("Mismatch of route db version");
                        }
                    } else {
                        warn!("Mismatch of nr_entries");
                    }
                } else {
                    let routes = known_routes
                        .iter()
                        .map(|e| (e.to, e.clone()))
                        .collect::<HashMap<Ipv4Addr, RouteInfo>>();
                    let peer_route_db = PeerRouteDB {
                        version: routedb_version,
                        nr_entries,
                        route_for: routes,
                    };
                    need_routes_update = nr_entries == peer_route_db.route_for.len();
                    self.peer_route_db.insert(sender, peer_route_db);
                }
            }
        }
        if need_routes_update {
            self.recalculate_routes();
        }
        need_routes_update
    }
    fn recalculate_routes(&mut self) {
        trace!("RECALC ROUTES");
        // Use as input:
        //    list of peers (being alive)
        //    peer route_db, if valid
        let mut new_routes: HashMap<Ipv4Addr, Option<Gateway>> = HashMap::new();

        for peer in self.peers.iter() {
            new_routes.insert(*peer, None);
        }
        for (wg_ip, peer_route_db) in self.peer_route_db.iter() {
            if peer_route_db.nr_entries == peer_route_db.route_for.len() {
                // is valid database for peer
                debug!("VALID {:?}", peer_route_db.route_for);
                for ri in peer_route_db.route_for.values() {
                    if ri.to == self.wg_ip {
                        continue;
                    }
                    if self.peers.contains(&ri.to) {
                        continue;
                    }
                    let mut hop_cnt = 1;
                    if let Some(gateway) = ri.gateway.as_ref() {
                        hop_cnt = gateway.hop_cnt + 1;
                        if gateway.ip == self.wg_ip {
                            continue;
                        }
                        if self.peers.contains(&gateway.ip) {
                            continue;
                        }
                        if self.peer_route_db.contains_key(&gateway.ip) {
                            continue;
                        }
                    }
                    // to-host can be reached via wg_ip
                    let gateway = Gateway {
                        ip: *wg_ip,
                        hop_cnt,
                    };
                    new_routes.insert(ri.to, Some(gateway));
                }
            } else {
                debug!("VALID {:?}", peer_route_db.route_for);
            }
        }

        for entry in new_routes.iter() {
            debug!("{:?}", entry);
        }

        // new_routes is built. So update route_db
        //
        // first routes to be deleted
        let mut changed = false;
        for ri in self.route_db.route_for.values_mut() {
            if !new_routes.contains_key(&ri.ri.to) {
                ri.to_be_deleted = true;
                changed = true;
            }
        }
        // finally routes to be updated / added
        for (to, gateway) in new_routes.into_iter() {
            let ng = gateway.clone().map(|mut gw| {
                gw.hop_cnt += 1;
                gw
            });
            match self.route_db.route_for.entry(to) {
                Entry::Vacant(e) => {
                    // new route
                    let ri = RouteInfoWithStatus {
                        ri: RouteInfo { to, gateway },
                        issued: false,
                        to_be_deleted: false,
                    };
                    e.insert(ri);
                    changed = true;
                }
                Entry::Occupied(mut e) => {
                    // update route
                    let current = e.get_mut();
                    let current_hop_cnt =
                        current.ri.gateway.as_ref().map(|e| e.hop_cnt).unwrap_or(0);
                    let new_hop_cnt = ng.as_ref().map(|e| e.hop_cnt).unwrap_or(0);
                    if current_hop_cnt > new_hop_cnt {
                        // new route is better
                        let ri = RouteInfoWithStatus {
                            ri: RouteInfo { to, gateway: ng },
                            issued: false,
                            to_be_deleted: false,
                        };
                        *current = ri;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.route_db.version += 1;
        }
    }
    pub fn get_ips_for_peer(&self, peer: Ipv4Addr) -> Vec<Ipv4Addr> {
        let mut ips = vec![];

        for ri in self.route_db.route_for.values() {
            if let Some(gateway) = ri.ri.gateway.as_ref() {
                if gateway.ip == peer {
                    ips.push(ri.ri.to);
                }
            }
        }

        ips
    }
}
