use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};

use log::*;

use crate::configuration::{PublicKeyWithTime, PublicPeer, StaticConfiguration};
use crate::crypt_udp::{AddressedTo, LocalContactPacket};
use crate::event::Event;
use crate::manager::RouteInfo;
use crate::wg_dev::map_to_ipv6;

pub trait NetParticipant {
    fn process_every_second(&mut self, now: u64, static_config: &StaticConfiguration)
        -> Vec<Event>;
    fn ok_to_delete_without_route(&self, now: u64) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct StaticPeer {
    peer: PublicPeer,
    is_alive: bool,
    send_advertisement_seconds_count_down: usize,
}
impl StaticPeer {
    pub fn from_public_peer(peer: &PublicPeer) -> Box<dyn NetParticipant> {
        Box::new(StaticPeer {
            peer: (*peer).clone(),
            is_alive: false,
            send_advertisement_seconds_count_down: 0,
        })
    }
}
impl NetParticipant for StaticPeer {
    fn process_every_second(
        &mut self,
        _now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        if !self.is_alive {
            if self.send_advertisement_seconds_count_down == 0 {
                self.send_advertisement_seconds_count_down = 60;
                // Resolve here to make it work for dyndns hosts
                match self.peer.endpoint.to_socket_addrs() {
                    Ok(endpoints) => {
                        trace!("ENDPOINTS: {:#?}", endpoints);
                        for sa in endpoints {
                            let destination = SocketAddr::new(sa.ip(), self.peer.admin_port);
                            events.push(Event::SendAdvertisement {
                                addressed_to: AddressedTo::StaticAddress,
                                to: destination,
                                wg_ip: self.peer.wg_ip,
                            });
                        }
                    }
                    Err(e) => {
                        // An error here is not dramatic. Just push out a warning and
                        // that's it
                        warn!(
                            "Cannot get endpoint ip(s) for {}: {:?}",
                            self.peer.endpoint, e
                        );
                    }
                }
            } else {
                self.send_advertisement_seconds_count_down -= 1;
            }
        }

        events
    }
}

#[derive(Debug)]
pub struct DynamicPeer {
    pub public_key: PublicKeyWithTime,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
    pub local_reachable_wg_endpoint: Option<SocketAddr>,
    pub local_reachable_admin_endpoint: Option<SocketAddr>,
    pub dp_visible_wg_endpoint: Option<SocketAddr>,
    pub admin_port: u16,
    pub lastseen: u64,
}
impl NetParticipant for DynamicPeer {
    fn process_every_second(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        // Pings are sent out only via the wireguard interface.
        //
        let dt = now - self.lastseen;
        if dt % 30 == 29 {
            let destination = SocketAddr::V4(SocketAddrV4::new(self.wg_ip, self.admin_port));
            events.push(Event::SendAdvertisement {
                addressed_to: AddressedTo::WireguardAddress,
                to: destination,
                wg_ip: self.wg_ip,
            });
        }
        events
    }
    fn ok_to_delete_without_route(&self, now: u64) -> bool {
        let dt = now - self.lastseen;
        dt > 120
    }
}

#[derive(Debug)]
pub struct Node {
    is_static_peer: Option<bool>,
    pub wg_ip: Ipv4Addr,
    admin_port: u16,
    //hop_cnt: usize,
    //gateway: Option<Ipv4Addr>,
    pub public_key: Option<PublicKeyWithTime>,
    known_in_s: usize,
    local_ip_list: Option<Vec<IpAddr>>,
    local_admin_port: Option<u16>,
    send_count: usize,
    can_send_to_visible_endpoint: bool,
    pub visible_endpoint: Option<SocketAddr>,
}
impl Node {
    pub fn from(ri: &RouteInfo) -> Self {
        Node {
            is_static_peer: None,
            wg_ip: ri.to,
            admin_port: ri.local_admin_port,
            //hop_cnt: ri.hop_cnt,
            //gateway: ri.gateway,
            public_key: None,
            known_in_s: 0,
            local_ip_list: None,
            local_admin_port: None,
            send_count: 0,
            can_send_to_visible_endpoint: false,
            visible_endpoint: None,
        }
    }
    pub fn process_local_contact(&mut self, local: LocalContactPacket) {
        self.local_ip_list = Some(local.local_ip_list);
        self.local_admin_port = Some(local.local_admin_port);
        self.visible_endpoint = local.my_visible_wg_endpoint;
        self.public_key = Some(local.public_key);
    }
}
impl NetParticipant for Node {
    fn process_every_second(
        &mut self,
        _now: u64,
        static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        let pk_available = if self.public_key.is_some() {
            ", public key available"
        } else {
            ""
        };
        trace!(target: "nodes", "Alive node: {:?} for {} s {}", self.wg_ip, self.known_in_s, pk_available);
        self.known_in_s += 1;

        if self.is_static_peer.is_none() {
            self.is_static_peer = Some(static_config.peers.contains_key(&self.wg_ip));
        }

        if self.is_static_peer == Some(true) {
            // nothing to do for a static peer
            //
            // static peers will be polled regularly until direct connection has been successfully
            // established.
            return events;
        }

        if self.local_ip_list.is_none()
            || self.public_key.is_none()
            || self.visible_endpoint.is_none()
        {
            if self.known_in_s % 60 == 0 || self.known_in_s < 5 {
                // Send request for local contact
                let destination = SocketAddrV4::new(self.wg_ip, self.admin_port);
                events.push(Event::SendLocalContactRequest { to: destination });
            }
        } else if let Some(admin_port) = self.local_admin_port.as_ref() {
            // All ok. so constantly send advertisement to the Ipv6 address
            events.push(Event::SendAdvertisement {
                addressed_to: AddressedTo::WireguardV6Address,
                to: SocketAddr::new(IpAddr::V6(map_to_ipv6(&self.wg_ip)), *admin_port),
                wg_ip: self.wg_ip,
            });
        }

        if self.send_count < 100 {
            if let Some(ip_list) = self.local_ip_list.as_ref() {
                if let Some(admin_port) = self.local_admin_port.as_ref() {
                    self.send_count += 1;
                    for ip in ip_list.iter() {
                        if let IpAddr::V4(ipv4) = ip {
                            if *ipv4 == self.wg_ip {
                                continue;
                            }
                            events.push(Event::SendAdvertisement {
                                addressed_to: AddressedTo::LocalAddress,
                                to: SocketAddr::new(*ip, *admin_port),
                                wg_ip: self.wg_ip,
                            });
                        }
                    }
                }
            }
        }

        let can_send = self.public_key.is_some() && self.visible_endpoint.is_some();

        if can_send && !self.can_send_to_visible_endpoint {
            self.can_send_to_visible_endpoint = true;
            events.push(Event::UpdateWireguardConfiguration);
        }

        events
    }
    fn ok_to_delete_without_route(&self, now: u64) -> bool {
        self.known_in_s > 10
    }
}
