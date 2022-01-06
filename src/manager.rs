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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use log::*;
use serde::{Deserialize, Serialize};

use crate::configuration::*;
use crate::crypt_udp::*;

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

#[derive(Debug)]
pub struct DynamicPeer {
    pub public_key: PublicKeyWithTime,
    pub local_ip_list: Vec<IpAddr>,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
    pub endpoint: Option<SocketAddr>,
    pub admin_port: u16,
    pub lastseen: u64,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    //all_nodes: HashMap<Ipv4Addr, NodeInfo>,
    peers: HashSet<Ipv4Addr>,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, PeerRouteDB>,

    pub peer: HashMap<Ipv4Addr, DynamicPeer>,
    pub fifo_dead: Vec<Ipv4Addr>,
    pub fifo_ping: Vec<Ipv4Addr>,
}

impl NetworkManager {
    pub fn new(wg_ip: Ipv4Addr) -> Self {
        NetworkManager {
            wg_ip,
            //all_nodes: HashMap::new(),
            peers: HashSet::new(),
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),

            peer: HashMap::new(),
            fifo_dead: vec![],
            fifo_ping: vec![],
        }
    }

    pub fn add_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        self.peers.insert(*peer_ip);
        self.recalculate_routes();
    }
    pub fn remove_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        self.peer.remove(peer_ip);
        self.peers.remove(peer_ip);
        self.recalculate_routes();
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
    pub fn analyze_advertisement_for_new_peer(
        &mut self,
        advertisement: &AdvertisementPacket,
        admin_port: u16,
    ) -> Option<Ipv4Addr> {
        self.fifo_dead.push(advertisement.wg_ip);
        self.fifo_ping.push(advertisement.wg_ip);
        let lastseen = crate::util::now();
        if self
            .peer
            .insert(
                advertisement.wg_ip,
                DynamicPeer {
                    wg_ip: advertisement.wg_ip,
                    local_ip_list: advertisement.local_ip_list.clone(),
                    local_admin_port: advertisement.local_admin_port,
                    local_wg_port: advertisement.local_wg_port,
                    public_key: advertisement.public_key.clone(),
                    name: advertisement.name.to_string(),
                    endpoint: advertisement.endpoint,
                    admin_port,
                    lastseen,
                },
            )
            .is_none()
        {
            Some(advertisement.wg_ip)
        } else {
            None
        }
    }
    pub fn analyze_advertisement(
        &mut self,
        advertisement: &AdvertisementPacket,
    ) -> Option<Ipv4Addr> {
        if let Some(peer_route_db) = self.peer_route_db.get(&advertisement.wg_ip) {
            if peer_route_db.version == advertisement.routedb_version {
                return None;
            }
            self.peer_route_db.remove(&advertisement.wg_ip);
        }
        Some(advertisement.wg_ip)
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
    pub fn process_route_database(&mut self, req: RouteDatabasePacket) -> bool {
        let mut need_routes_update = false;
        debug!("RouteDatabase from {}: {:?}", req.sender, req.known_routes);
        if let Some(mut peer_route_db) = self.peer_route_db.remove(&req.sender) {
            if req.nr_entries == peer_route_db.nr_entries {
                if req.routedb_version == peer_route_db.version {
                    for ri in req.known_routes {
                        peer_route_db.route_for.insert(ri.to, ri);
                    }
                    need_routes_update = req.nr_entries == peer_route_db.route_for.len();
                    self.peer_route_db.insert(req.sender, peer_route_db);
                } else {
                    warn!("Mismatch of route db version");
                }
            } else {
                warn!("Mismatch of nr_entries");
            }
        } else {
            let routes = req
                .known_routes
                .iter()
                .map(|e| (e.to, e.clone()))
                .collect::<HashMap<Ipv4Addr, RouteInfo>>();
            let peer_route_db = PeerRouteDB {
                version: req.routedb_version,
                nr_entries: req.nr_entries,
                route_for: routes,
            };
            need_routes_update = req.nr_entries == peer_route_db.route_for.len();
            self.peer_route_db.insert(req.sender, peer_route_db);
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

        // Dynamic peers are ALWAYS reachable without a gateway
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
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.peer.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for peer in self.peer.values() {
            info!("{:?}", peer);
        }
    }
    pub fn check_timeouts(&mut self, limit: u64) -> HashSet<Ipv4Addr> {
        let mut dead_peers = HashSet::new();
        let now = crate::util::now();
        while let Some(wg_ip) = self.fifo_dead.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                let dt = now - peer.lastseen;
                trace!(target: "dead_peer", "Peer last seen {} s before, limit = {}", dt, limit);
                if dt < limit {
                    break;
                }
                dead_peers.insert(**wg_ip);
            }
            self.fifo_dead.remove(0);
        }
        dead_peers
    }
    pub fn check_ping_timeouts(&mut self, limit: u64) -> HashSet<(Ipv4Addr, u16)> {
        let mut ping_peers = HashSet::new();
        let now = crate::util::now();
        while let Some(wg_ip) = self.fifo_ping.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                let dt = now - peer.lastseen;
                if dt < limit {
                    break;
                }
                ping_peers.insert((**wg_ip, peer.admin_port));
            }
            self.fifo_ping.remove(0);
        }
        ping_peers
    }
}
