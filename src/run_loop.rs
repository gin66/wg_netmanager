use std::net::{IpAddr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;

use log::*;

use crate::arch_def::Architecture;
use crate::configuration::*;
use crate::crypt_udp::CryptUdp;
use crate::crypt_udp::UdpPacket;
use crate::error::*;
use crate::event::Event;
use crate::manager::*;
use crate::node::*;
use crate::tui_display::TuiApp;
use crate::wg_dev::*;
use crate::Arch;

pub fn run(
    static_config: &StaticConfiguration,
    mut wg_dev: Box<dyn WireguardDevice>,
) -> BoxResult<()> {
    let (tx, rx) = channel();

    Arch::arch_specific_init(tx.clone());

    let tx_handler = tx.clone();
    ctrlc::set_handler(move || {
        warn!("CTRL-C");
        tx_handler
            .send(Event::CtrlC)
            .expect("Could not send signal on channel.")
    })
    .expect("Error setting Ctrl-C handler");

    let port = static_config.my_admin_port();

    let (v4_socket_first, need_v4_socket, need_v6_socket) = Arch::ipv4v6_socket_setup();

    let mut opt_crypt_socket_v6 = None;
    let mut opt_crypt_socket_v4 = None;

    if need_v4_socket && v4_socket_first {
        debug!("bind to 0.0.0.0:{}", port);
        opt_crypt_socket_v4 = Some(
            CryptUdp::bind(IpAddr::V4("0.0.0.0".parse().unwrap()), port)?
                .key(&static_config.shared_key)?,
        );
    }
    if need_v6_socket {
        debug!("bind to :::{}", port);
        opt_crypt_socket_v6 = Some(
            CryptUdp::bind(IpAddr::V6("::".parse().unwrap()), port)?
                .key(&static_config.shared_key)?,
        );
    }
    if need_v4_socket && !v4_socket_first {
        debug!("bind to 0.0.0.0:{}", port);
        opt_crypt_socket_v4 = Some(
            CryptUdp::bind(IpAddr::V4("0.0.0.0".parse().unwrap()), port)?
                .key(&static_config.shared_key)?,
        );
    }

    if opt_crypt_socket_v4.is_none() {
        opt_crypt_socket_v4 = opt_crypt_socket_v6.as_ref().map(|s| s.try_clone().unwrap());
    }
    if opt_crypt_socket_v6.is_none() {
        opt_crypt_socket_v6 = opt_crypt_socket_v4.as_ref().map(|s| s.try_clone().unwrap());
    }

    let crypt_socket_v4 = opt_crypt_socket_v4.unwrap();
    let crypt_socket_v6 = opt_crypt_socket_v6.unwrap();

    // Set up udp receiver thread for ipv4
    if need_v4_socket {
        let tx_clone = tx.clone();
        let crypt_socket_v4_clone = crypt_socket_v4
            .try_clone()
            .expect("couldn't clone the crypt_socket");
        std::thread::spawn(move || loop {
            let mut buf = [0; 2000];
            match crypt_socket_v4_clone.recv_from(&mut buf) {
                Ok((received, src_addr)) => {
                    info!("received {} bytes from {:?}", received, src_addr);
                    match rmp_serde::from_slice::<UdpPacket>(&buf[..received]) {
                        Ok(udp_packet) => {
                            tx_clone.send(Event::Udp(udp_packet, src_addr)).unwrap();
                        }
                        Err(e) => {
                            error!("Error in json decode: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        });
    }

    // Set up udp receiver thread for ipv6
    if need_v6_socket {
        let tx_clone = tx.clone();
        let crypt_socket_v6_clone = crypt_socket_v6
            .try_clone()
            .expect("couldn't clone the crypt_socket");
        std::thread::spawn(move || loop {
            let mut buf = [0; 2000];
            match crypt_socket_v6_clone.recv_from(&mut buf) {
                Ok((received, src_addr)) => {
                    info!("received {} bytes from {:?}", received, src_addr);
                    match rmp_serde::from_slice::<UdpPacket>(&buf[..received]) {
                        Ok(udp_packet) => {
                            tx_clone.send(Event::Udp(udp_packet, src_addr)).unwrap();
                        }
                        Err(e) => {
                            error!("Error in json decode: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        });
    }

    // Set up timer tick
    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let interval_1s = time::Duration::from_millis(1000);
        loop {
            tx_clone.send(Event::TimerTick1s).unwrap();
            std::thread::sleep(interval_1s);
        }
    });

    // in case there are dangling routes
    if !static_config.use_existing_interface {
        wg_dev.take_down_device().ok();

        wg_dev.create_device()?;
    } else {
        wg_dev.flush_all()?;
    }

    wg_dev.set_ip(&static_config.wg_ip, &static_config.subnet)?;

    let mut tui_app = if static_config.use_tui {
        TuiApp::init(tx.clone())?
    } else {
        TuiApp::off()
    };

    let rc = main_loop(
        static_config,
        &*wg_dev,
        crypt_socket_v4,
        crypt_socket_v6,
        tx,
        rx,
        &mut tui_app,
    );

    if !static_config.use_existing_interface {
        wg_dev.take_down_device().ok();
    }

    tui_app.deinit()?;

    rc
}

fn main_loop(
    static_config: &StaticConfiguration,
    wg_dev: &dyn WireguardDevice,
    mut crypt_socket_v4: CryptUdp,
    mut crypt_socket_v6: CryptUdp,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    tui_app: &mut TuiApp,
) -> BoxResult<()> {
    let mut network_manager = NetworkManager::new(static_config);

    // set up initial wireguard configuration without peers
    tx.send(Event::UpdateWireguardConfiguration).unwrap();

    let mut tick_cnt = 0;
    loop {
        trace!(target: "loop", "Main loop");
        let evt = rx.recv();
        trace!(target: "loop", "{:?}", evt);
        match evt {
            Err(e) => {
                error!("Receive error: {:?}", e);
                break;
            }
            Ok(Event::CtrlC) => {
                break;
            }
            Ok(Event::TimerTick1s) => {
                tui_app.draw()?;

                if tick_cnt % 30 == 2 {
                    // every 30s
                    network_manager.stats();
                }

                let events = network_manager.process_all_nodes_every_second(static_config);
                for evt in events.into_iter() {
                    tx.send(evt).unwrap();
                }

                tick_cnt += 1;
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                let src_addr = match src_addr {
                    SocketAddr::V4(_) => src_addr,
                    SocketAddr::V6(sa) => {
                        if let Some(ipv4) = sa.ip().to_ipv4() {
                            SocketAddr::V4(SocketAddrV4::new(ipv4, sa.port()))
                        } else {
                            src_addr
                        }
                    }
                };

                use UdpPacket::*;
                let events: Vec<Event>;
                match udp_packet {
                    Advertisement(ad) => {
                        debug!(target: &ad.wg_ip.to_string(), "Received advertisement from {:?}", src_addr);
                        events = network_manager.analyze_advertisement(static_config, ad, src_addr);
                    }
                    RouteDatabaseRequest => match src_addr {
                        SocketAddr::V4(destination) => {
                            info!(target: "routing", "RouteDatabaseRequest from {:?}", src_addr);
                            debug!(target: &destination.ip().to_string(), "Received database request");
                            events = vec![Event::SendRouteDatabase { to: destination }];
                        }
                        SocketAddr::V6(source) => {
                            error!(target: "routing", "Expected IPV4 and not IPV6 address {:?}", source);
                            events = vec![];
                        }
                    },
                    RouteDatabase(db) => {
                        info!(target: "routing", "RouteDatabase from {}", src_addr);
                        debug!(target: &src_addr.ip().to_string(), "Received route database, version = {}", db.routedb_version);
                        events = network_manager.process_route_database(db);
                    }
                    LocalContactRequest => match src_addr {
                        SocketAddr::V4(destination) => {
                            info!(target: "probing", "LocalContactRequest from {:?}", src_addr);
                            debug!(target: &destination.ip().to_string(), "Received local contact request");
                            events = vec![Event::SendLocalContact { to: destination }];
                        }
                        SocketAddr::V6(source) => {
                            error!(target: "probing", "Expected IPV4 and not IPV6 address {:?}", source);
                            events = vec![];
                        }
                    },
                    LocalContact(contact) => {
                        debug!(target: "probing", "Received contact info: {:#?}", contact);
                        debug!(target: &contact.wg_ip.to_string(), "Received local contacts");
                        network_manager.process_local_contact(contact);
                        events = vec![];
                    }
                }
                for evt in events {
                    tx.send(evt).unwrap();
                }
            }
            Ok(Event::SendAdvertisement {
                addressed_to,
                to: destination,
                wg_ip,
            }) => {
                debug!(target: &wg_ip.to_string(),"Send advertisement to {:?}", destination);
                let routedb_version = network_manager.db_version();
                let my_visible_wg_endpoint =
                    network_manager.my_visible_wg_endpoint.as_ref().copied();
                let opt_dp: Option<&DynamicPeer> = network_manager.dynamic_peer_for(&wg_ip);
                let advertisement = UdpPacket::advertisement_from_config(
                    static_config,
                    routedb_version,
                    addressed_to,
                    opt_dp,
                    my_visible_wg_endpoint,
                );
                let buf = rmp_serde::to_vec(&advertisement).unwrap();
                info!(target: "advertisement", "Send advertisement to {}", destination);
                if destination.is_ipv4() {
                    crypt_socket_v4.send_to(&buf, destination).ok();
                } else {
                    crypt_socket_v6.send_to(&buf, destination).ok();
                }
            }
            Ok(Event::SendRouteDatabaseRequest { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send route database request to {:?}", destination);
                let request = UdpPacket::route_database_request();
                let buf = rmp_serde::to_vec(&request).unwrap();
                info!(target: "routing", "Send RouteDatabaseRequest to {}", destination);
                crypt_socket_v4
                    .send_to(&buf, SocketAddr::V4(destination))
                    .ok();
            }
            Ok(Event::SendRouteDatabase { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send route database to {:?}", destination);
                let packages = network_manager.provide_route_database();
                for p in packages {
                    let buf = rmp_serde::to_vec(&p).unwrap();
                    info!(target: "routing", "Send RouteDatabase to {}", destination);
                    crypt_socket_v4
                        .send_to(&buf, SocketAddr::V4(destination))
                        .ok();
                }
            }
            Ok(Event::SendLocalContactRequest { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send local contact request to {:?}", destination);
                let request = UdpPacket::local_contact_request();
                let buf = rmp_serde::to_vec(&request).unwrap();
                info!(target: "probing", "Send LocalContactRequest to {}", destination);
                crypt_socket_v4
                    .send_to(&buf, SocketAddr::V4(destination))
                    .ok();
            }
            Ok(Event::SendLocalContact { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send local contacts to {:?}", destination);
                let local_contact = UdpPacket::local_contact_from_config(
                    static_config,
                    network_manager.my_visible_wg_endpoint,
                );
                trace!(target: "probing", "local contact to {:#?}", local_contact);
                let buf = rmp_serde::to_vec(&local_contact).unwrap();
                info!(target: "probing", "Send local contact to {}", destination);
                crypt_socket_v4
                    .send_to(&buf, SocketAddr::V4(destination))
                    .ok();
            }
            Ok(Event::UpdateWireguardConfiguration) => {
                info!("Update peers");
                let conf = static_config.to_wg_configuration(&network_manager);
                info!(target: "wireguard", "Configuration as peer\n{}\n", conf);
                wg_dev.sync_conf(&conf)?;
            }
            Ok(Event::ReadWireguardConfiguration) => {
                let pubkey_to_endpoint = wg_dev.retrieve_conf()?;
                network_manager.current_wireguard_configuration(pubkey_to_endpoint);
            }
            Ok(Event::UpdateRoutes) => {
                let changes = network_manager.get_route_changes();
                for rc in changes {
                    use RouteChange::*;
                    debug!("{:?}", rc);
                    match rc {
                        AddRoute { to, gateway } => {
                            debug!(target: &to.to_string(), "add route with gateway {:?}", gateway);
                            wg_dev.add_route(to, gateway)?;
                        }
                        ReplaceRoute { to, gateway } => {
                            debug!(target: &to.to_string(), "replace route with gateway {:?}", gateway);
                            wg_dev.replace_route(to, gateway)?;
                        }
                        DelRoute { to, gateway } => {
                            debug!(target: &to.to_string(), "del route with gateway {:?}", gateway);
                            wg_dev.del_route(to, gateway)?;
                        }
                    }
                }
                tx.send(Event::UpdateWireguardConfiguration).unwrap();
            }
            Ok(Event::TuiApp(evt)) => {
                tui_app.process_event(evt);
                tui_app.draw()?;
            }
        }
    }
    Ok(())
}
