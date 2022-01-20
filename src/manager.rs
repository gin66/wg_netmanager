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
use std::net::{Ipv4Addr, SocketAddr};

use log::*;

use crate::configuration::*;
use crate::crypt_udp::*;
use crate::event::Event;
use crate::node::*;

#[derive(Debug)]
pub enum RouteChange {
    AddRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
    ReplaceRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
    DelRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
}

#[derive(Default, Debug)]
pub struct RouteDB {
    version: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    pub my_visible_wg_endpoint: Option<SocketAddr>,
    route_db: RouteDB,
    pub all_nodes: HashMap<Ipv4Addr, Box<dyn Node>>,
}

impl NetworkManager {
    pub fn new(static_config: &StaticConfiguration) -> Self {
        let all_nodes = static_config
            .peers
            .iter()
            .filter(|(wg_ip, _)| **wg_ip != static_config.wg_ip)
            .map(|(wg_ip, peer)| (*wg_ip, StaticPeer::from_public_peer(peer)))
            .collect::<HashMap<Ipv4Addr, Box<dyn Node>>>();

        NetworkManager {
            wg_ip: static_config.wg_ip,
            my_visible_wg_endpoint: None,
            route_db: RouteDB::default(),
            all_nodes,
        }
    }

    pub fn db_version(&self) -> usize {
        self.route_db.version
    }
    pub fn stats(&self) {
        trace!("Manager: {} nodes in network", self.all_nodes.len(),);
    }
    pub fn analyze_advertisement(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> Vec<Event> {
        if let Some(endpoint) = advertisement.your_visible_wg_endpoint.as_ref() {
            // Could be more than one
            self.my_visible_wg_endpoint = Some(*endpoint);
        }

        match self.all_nodes.entry(advertisement.wg_ip) {
            Entry::Occupied(mut entry) => {
                let now = crate::util::now();
                let (opt_new_entry, events) = entry.get_mut().analyze_advertisement(
                    now,
                    static_config,
                    advertisement,
                    src_addr,
                );
                if let Some(new_entry) = opt_new_entry {
                    entry.insert(new_entry);
                }
                events
            }
            Entry::Vacant(entry) => {
                let mut events = vec![];
                info!(target: "advertisement", "Advertisement from new peer {}", src_addr);

                events.push(Event::UpdateWireguardConfiguration);

                // Answers to advertisments are only sent, if the wireguard ip is not
                // in the list of dynamic peers and as such is new.
                // Consequently the reply is sent over the internet and not via
                // wireguard tunnel, because that tunnel is not yet set up.
                events.push(Event::SendAdvertisement {
                    addressed_to: advertisement.addressed_to.reply(),
                    to: src_addr,
                    wg_ip: self.wg_ip,
                });
                events.push(Event::UpdateRoutes);

                let dp =
                    DynamicPeer::from_advertisement(now, static_config, advertisement, src_addr);
                entry.insert(Box::new(dp));

                events
            }
        }
    }
    pub fn process_all_nodes_every_second(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];
        let mut node_to_delete = vec![];
        for (node_wg_ip, node) in self.all_nodes.iter_mut() {
            //    if !self.route_db.route_for.contains_key(node_wg_ip) {
            // have no route to this peer
            if node.ok_to_delete_without_route(now) {
                node_to_delete.push(*node_wg_ip);
                continue;
            }
            //    }
            let mut new_events = node.process_every_second(now, static_config);
            events.append(&mut new_events);
        }

        if !node_to_delete.is_empty() {
            events.push(Event::UpdateWireguardConfiguration);
            events.push(Event::UpdateRoutes);

            for wg_ip in node_to_delete {
                debug!(target: &wg_ip.to_string(), "is dead => remove");
                debug!(target: "dead_peer", "Found dead peer {}", wg_ip);
                self.all_nodes.remove(&wg_ip);
            }
        }

        events
    }
    pub fn provide_route_database(&self) -> Vec<UdpPacket> {
        let mut known_routes = vec![];
        for ri in self.route_db.route_for.values() {
            known_routes.push(ri);
        }
        let p = UdpPacket::make_route_database(
            self.wg_ip,
            self.route_db.version,
            known_routes.len(),
            known_routes,
        );
        vec![p]
    }
    pub fn process_route_database(&mut self, req: RouteDatabasePacket) -> Option<Vec<Event>> {
        debug!(target: "routing", "RouteDatabase: {:#?}", req.known_routes);

        self.all_nodes
            .get_mut(&req.sender)
            .and_then(|node| node.process_route_database(req))
    }
    pub fn process_local_contact(&mut self, local: LocalContactPacket) {
        // Send advertisement to all local addresses
        debug!(target: &local.wg_ip.to_string(), "LocalContact: {:#?}", local);
        let wg_ip = local.wg_ip;
        if let Some(node) = self.all_nodes.get_mut(&wg_ip) {
            node.process_local_contact(local);
        }
    }
    pub fn get_route_changes(&mut self) -> Vec<RouteChange> {
        let mut route_changes = vec![];
        trace!(target: "routing", "Recalculate routes");
        let mut new_routes: HashMap<Ipv4Addr, RouteInfo> = HashMap::new();

        for (wg_ip, node) in self.all_nodes.iter() {
            if node.is_distant_node() {
                continue;
            }
            trace!(target: "routing", "Include direct path to static/dynamic peer to new routes: {}", wg_ip);
            let ri = RouteInfo {
                to: *wg_ip,
                local_admin_port: node.local_admin_port(),
                hop_cnt: 0,
                gateway: None,
            };
            new_routes.insert(*wg_ip, ri);
        }
        // Then add all indirect routes from the node's routedb

        let mut new_nodes = vec![];
        for (wg_ip, node) in self.all_nodes.iter() {
            if let Some(routedb) = node.routedb_manager().and_then(|mgr| mgr.routedb.as_ref()) {
                for ri in routedb.route_for.values() {
                    if ri.to == self.wg_ip {
                        trace!(target: "routing", "Route to myself => ignore");
                        continue;
                    }
                    let mut hop_cnt = 1;
                    if let Some(gateway) = ri.gateway.as_ref() {
                        // Ignore routes to myself as gateway
                        if *gateway == self.wg_ip {
                            trace!(target: "routing", "Route to myself as gateway => ignore");
                            continue;
                        }
                        if self.all_nodes.get(gateway).map(|n| n.is_distant_node()) != Some(true) {
                            trace!(target: "routing", "Route using any of my peers as gateway => ignore");
                            continue;
                        }

                        hop_cnt = ri.hop_cnt + 1;
                    }

                    // to-host can be reached via wg_ip
                    trace!(target: "routing", "Include to routes: {} via {:?} and hop_cnt {}", ri.to, wg_ip, hop_cnt);
                    let ri_new = RouteInfo {
                        to: ri.to,
                        local_admin_port: ri.local_admin_port,
                        hop_cnt,
                        gateway: Some(*wg_ip),
                    };
                    match new_routes.entry(ri.to) {
                        Entry::Vacant(e) => {
                            e.insert(ri_new);
                        }
                        Entry::Occupied(mut e) => {
                            let current = e.get_mut();
                            if current.hop_cnt > ri_new.hop_cnt {
                                // new route is better, so replace
                                *current = ri_new;
                            }
                        }
                    }
                    if !self.all_nodes.contains_key(&ri.to) {
                        info!(target: "probing", "detected a new node {} via {:?}", ri.to, ri.gateway);
                        let node = DistantNode::from(ri);
                        new_nodes.push((ri.to, node));
                    }
                }
            }
        }
        for (wg_ip, node) in new_nodes {
            self.all_nodes.insert(wg_ip, Box::new(node));
        }

        for entry in new_routes.iter() {
            debug!(target: "routing", "new routes' entry: {:?}", entry);
        }
        for ri in self.route_db.route_for.values_mut() {
            debug!(target: "routing", "Existing route: {:?}", ri);
        }

        // new_routes is built. Update gateway info of all nodes
        for (wg_ip, node) in self.all_nodes.iter_mut() {
            let gateway = new_routes.get(wg_ip).and_then(|ri| ri.gateway);
            node.set_gateway(gateway);
            node.clear_gateway_for();
        }
        for ri in new_routes.values() {
            if let Some(gateway) = ri.gateway.as_ref() {
                if let Some(node) = self.all_nodes.get_mut(gateway) {
                    warn!("gateway for {} {}", ri.to, gateway);
                    node.add_gateway_for(ri.to);
                }
            }
        }

        // remove all distant nodes without a route
        self.all_nodes
            .retain(|wg_ip, node| !node.is_distant_node() || new_routes.contains_key(wg_ip));

        // So update route_db and mark changes
        //
        // first routes to be deleted
        let mut to_be_deleted = vec![];
        for ri in self.route_db.route_for.values_mut() {
            if !new_routes.contains_key(&ri.to) {
                trace!(target: "routing", "del route {:?}", ri);
                route_changes.push(RouteChange::DelRoute {
                    to: ri.to,
                    gateway: ri.gateway,
                });

                to_be_deleted.push(ri.to);

                // and delete from the known_nodes.
                //self.known_nodes.remove(&ri.to);
            } else {
                trace!(target: "routing", "unchanged route {:?}", ri);
            }
        }
        for wg_ip in to_be_deleted.into_iter() {
            self.route_db.route_for.remove(&wg_ip);
        }
        // finally routes to be updated / added
        for (to, ri) in new_routes.into_iter() {
            trace!(target: "routing", "process route {} via {:?}", to, ri.gateway);
            match self.route_db.route_for.entry(to) {
                Entry::Vacant(e) => {
                    // new node with route
                    trace!(target: "routing", "is new route {} via {:?}", to, ri.gateway);
                    route_changes.push(RouteChange::AddRoute {
                        to,
                        gateway: ri.gateway,
                    });
                    let mut ri_new = RouteInfo {
                        to,
                        local_admin_port: ri.local_admin_port,
                        hop_cnt: ri.hop_cnt,
                        gateway: ri.gateway,
                    };
                    if ri.gateway.is_some() {
                        ri_new.hop_cnt += 1;
                    }
                    e.insert(ri_new);
                }
                Entry::Occupied(mut e) => {
                    // update route
                    if e.get().to != ri.to || e.get().gateway != ri.gateway {
                        trace!(target: "routing", "replace existing route {}", to);
                        route_changes.push(RouteChange::ReplaceRoute {
                            to,
                            gateway: ri.gateway,
                        });
                        *e.get_mut() = RouteInfo {
                            to,
                            local_admin_port: ri.local_admin_port,
                            hop_cnt: ri.hop_cnt,
                            gateway: ri.gateway,
                        };
                    }
                }
            }
            trace!(target: "routing", "route changes: {}", route_changes.len());
        }
        if !route_changes.is_empty() {
            trace!(target: "routing", "{} route changes", route_changes.len());
            for change in route_changes.iter() {
                trace!(target: "routing", "route changes {:?}", change);
            }
            self.route_db.version += 1;
        }
        route_changes
    }
    pub fn get_ips_for_peer(&self, peer: Ipv4Addr) -> Vec<Ipv4Addr> {
        let mut ips = vec![];

        for ri in self.route_db.route_for.values() {
            if ri.gateway == Some(peer) {
                ips.push(ri.to);
            }
        }

        ips
    }
    pub fn node_for(&mut self, wg_ip: &Ipv4Addr) -> Option<&dyn Node> {
        self.all_nodes.get(wg_ip).map(|n| n.as_ref())
    }
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.all_nodes.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for wg_ip in self.all_nodes.keys() {
            debug!(target: "nodes", "{:?}", wg_ip);
        }
    }
    pub fn current_wireguard_configuration(
        &mut self,
        mut pubkey_to_endpoint: HashMap<String, SocketAddr>,
    ) {
        for node in self.all_nodes.values_mut() {
            node.update_from_wireguard_configuration(&mut pubkey_to_endpoint);
        }
    }
}
