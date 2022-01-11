use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;

use clap::{App, Arg};
use log::*;
use yaml_rust::YamlLoader;

use wg_netmanager::configuration::*;
use wg_netmanager::crypt_udp::CryptUdp;
use wg_netmanager::crypt_udp::UdpPacket;
use wg_netmanager::error::*;
use wg_netmanager::event::Event;
use wg_netmanager::manager::*;
use wg_netmanager::tui_display::TuiApp;
use wg_netmanager::wg_dev::*;

fn main() -> BoxResult<()> {
    let matches = App::new("Wireguard Network Manager")
        .version("0.1")
        .author("Jochen Kiemes <jochen@kiemes.de>")
        .about("Manages a network of wireguard nodes with no central server.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Custom config file in yaml-style")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("existing_interface")
                .short("e")
                .long("existing_wg")
                .help("Use an existing wireguard interface and do not try to create one"),
        )
        .arg(
            Arg::with_name("wireguard_port")
                .short("w")
                .long("wireguard-port")
                .value_name("PORT")
                .default_value("50001")
                .help("Wireguard udp port aka Listen port, if not defined in config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("admin_port")
                .short("u")
                .long("admin-port")
                .value_name("PORT")
                .default_value("55551")
                .help("udp port for encrypted communication, if not defined in config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("tui")
                .short("t")
                .help("Use text user interface"),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("interface")
                .help("Sets the wireguard interface")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("wg_ip")
                .help("Sets the wireguard private ip")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::with_name("name")
                .help("Sets the name for this computer")
                .required(true)
                .index(3),
        )
        .get_matches();

    let use_tui = matches.is_present("tui");
    let use_existing_interface = matches.is_present("existing_interface");

    // Select logger based on command line flag
    //
    if use_tui {
        tui_logger::init_logger(log::LevelFilter::Trace).unwrap();
        tui_logger::set_default_level(log::LevelFilter::Trace);
    } else {
        let log_filter = match matches.occurrences_of("v") {
            0 => log::LevelFilter::Error,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        set_up_logging(log_filter);
    }

    let interface = matches.value_of("interface").unwrap();
    let wg_ip: Ipv4Addr = matches.value_of("wg_ip").unwrap().parse().unwrap();
    let wg_port: u16 = matches.value_of("wireguard_port").unwrap().parse().unwrap();
    let admin_port: u16 = matches.value_of("admin_port").unwrap().parse().unwrap();
    let computer_name = matches.value_of("name").unwrap();
    #[cfg(target_os = "linux")]
    let ip_list = wg_netmanager::interfaces::get();
    #[cfg(not(target_os = "linux"))]
    let ip_list = vec![];

    let config = matches.value_of("config").unwrap_or("network.yaml");

    let mut file = File::open(config)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let conf = YamlLoader::load_from_str(&content).unwrap();

    debug!("Raw configuration:");
    debug!("{:#?}", conf);

    let network = &conf[0]["network"];
    let shared_key = base64::decode(&network["sharedKey"].as_str().unwrap()).unwrap();

    let mut peers: HashMap<Ipv4Addr, PublicPeer> = HashMap::new();
    for p in conf[0]["peers"].as_vec().unwrap() {
        info!("STATIC PEER: {:#?}", p);
        let wg_port = p["wgPort"].as_i64().unwrap() as u16;
        let endpoint = p["endPoint"].as_str().unwrap().to_string();
        let admin_port = p["adminPort"].as_i64().unwrap() as u16;
        let wg_ip: Ipv4Addr = p["wgIp"].as_str().unwrap().parse().unwrap();
        let pp = PublicPeer {
            endpoint,
            wg_port,
            admin_port,
            wg_ip,
        };
        peers.insert(wg_ip, pp);
    }

    let output = Command::new("wg")
        .arg("genkey")
        .stdout(Stdio::piped())
        .output()?
        .stdout;
    let raw_private_key = String::from_utf8_lossy(&output);
    let my_private_key = raw_private_key.trim();
    trace!("My private key: {}", my_private_key);
    let mut cmd = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    write!(cmd.stdin.as_ref().unwrap(), "{}", my_private_key)?;
    cmd.wait()?;
    let mut public_key = String::new();
    cmd.stdout.unwrap().read_to_string(&mut public_key)?;
    let my_public_key = public_key.trim();
    let timestamp = wg_netmanager::util::now();
    let my_public_key_with_time = PublicKeyWithTime {
        key: my_public_key.to_string(),
        priv_key_creation_time: timestamp,
    };
    trace!("My public key: {}", my_public_key);

    let static_config = StaticConfiguration::builder()
        .name(computer_name)
        .ip_list(ip_list)
        .wg_ip(wg_ip)
        .wg_name(interface)
        .wg_port(wg_port)
        .admin_port(admin_port)
        .shared_key(shared_key)
        .my_public_key(my_public_key_with_time)
        .my_private_key(my_private_key)
        .peers(peers)
        .use_tui(use_tui)
        .use_existing_interface(use_existing_interface)
        .build();

    run(&static_config)
}

fn run(static_config: &StaticConfiguration) -> BoxResult<()> {
    let (tx, rx) = channel();

    let tx_handler = tx.clone();
    ctrlc::set_handler(move || {
        warn!("CTRL-C");
        tx_handler
            .send(Event::CtrlC)
            .expect("Could not send signal on channel.")
    })
    .expect("Error setting Ctrl-C handler");

    let port = static_config.my_admin_port();
    debug!("bind to 0.0.0.0:{}", port);
    let crypt_socket = CryptUdp::bind(port)?.key(&static_config.shared_key)?;

    // Set up udp receiver thread
    let tx_clone = tx.clone();
    let crypt_socket_clone = crypt_socket
        .try_clone()
        .expect("couldn't clone the crypt_socket");
    std::thread::spawn(move || loop {
        let mut buf = [0; 2000];
        match crypt_socket_clone.recv_from(&mut buf) {
            Ok((received, src_addr)) => {
                info!("received {} bytes from {:?}", received, src_addr);
                match serde_json::from_slice::<UdpPacket>(&buf[..received]) {
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

    // Set up timer tick
    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let interval_1s = time::Duration::from_millis(1000);
        loop {
            tx_clone.send(Event::TimerTick1s).unwrap();
            std::thread::sleep(interval_1s);
        }
    });

    let mut wg_dev = WireguardDeviceLinux::init(&static_config.wg_name);

    // in case there are dangling routes
    if !static_config.use_existing_interface {
        wg_dev.take_down_device().ok();

        wg_dev.bring_up_device()?;
    } else {
        wg_dev.flush_all()?;
    }

    wg_dev.set_ip(&static_config.wg_ip)?;

    let mut tui_app = if static_config.use_tui {
        TuiApp::init(tx.clone())?
    } else {
        TuiApp::off()
    };

    let rc = main_loop(static_config, &wg_dev, crypt_socket, tx, rx, &mut tui_app);

    if !static_config.use_existing_interface {
        wg_dev.take_down_device().ok();
    }

    tui_app.deinit()?;

    rc
}

fn main_loop(
    static_config: &StaticConfiguration,
    wg_dev: &dyn WireguardDevice,
    mut crypt_socket: CryptUdp,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    tui_app: &mut TuiApp,
) -> BoxResult<()> {
    let mut network_manager = NetworkManager::new(static_config.wg_ip);

    // set up initial wireguard configuration without peers
    tx.send(Event::UpdateWireguardConfiguration).unwrap();
    tx.send(Event::SendAdvertisementToPublicPeers).unwrap();

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

                if tick_cnt % 15 == 1 {
                    // every 15s
                    tx.send(Event::CheckAndRemoveDeadDynamicPeers).unwrap();
                }
                if tick_cnt % 5 == 0 {
                    // every 20s
                    tx.send(Event::SendPingToAllDynamicPeers).unwrap();
                }
                if tick_cnt % 30 == 2 {
                    // every 30s
                    network_manager.stats();
                }
                if tick_cnt % 60 == 3 {
                    // every 60s
                    tx.send(Event::SendAdvertisementToPublicPeers).unwrap();
                }
                tick_cnt += 1;
            }
            Ok(Event::SendPingToAllDynamicPeers) => {
                // Pings are sent out only via the wireguard interface.
                //
                let ping_peers = network_manager.check_ping_timeouts(10); // should be < half of dead peer timeout
                for (wg_ip, admin_port) in ping_peers {
                    let destination = SocketAddr::V4(SocketAddrV4::new(wg_ip, admin_port));
                    tx.send(Event::SendAdvertisement {
                        to: destination,
                        wg_ip,
                    })
                    .unwrap();
                }
            }
            Ok(Event::SendAdvertisementToPublicPeers) => {
                // These advertisements are sent to the known internet address as defined in the config file.
                // As all udp packets are encrypted, this should not be an issue.
                //
                for peer in static_config.peers.values() {
                    if !network_manager.knows_peer(&peer.wg_ip) {
                        // ensure not to send to myself
                        if peer.wg_ip != static_config.wg_ip {
                            // Resolve here to make it work for dyndns hosts
                            let endpoints = peer.endpoint.to_socket_addrs()?;
                            trace!("ENDPOINTS: {:#?}", endpoints);
                            for sa in endpoints {
                                let destination = SocketAddr::new(sa.ip(), peer.admin_port);
                                tx.send(Event::SendAdvertisement {
                                    to: destination,
                                    wg_ip: peer.wg_ip,
                                })
                                .unwrap();
                            }
                        }
                    }
                }
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                use UdpPacket::*;
                let events: Vec<Event>;
                match udp_packet {
                    Advertisement(ad) => {
                        debug!(target: &ad.wg_ip.to_string(), "Received advertisement");
                        events = network_manager.analyze_advertisement(ad, src_addr);
                    }
                    RouteDatabaseRequest => match src_addr {
                        SocketAddr::V4(destination) => {
                            info!(target: "routing", "RouteDatabaseRequest from {:?}", src_addr);
                            debug!(target: &destination.ip().to_string(), "Received database request");
                            events = vec![Event::SendRouteDatabase { to: destination }];
                        }
                        SocketAddr::V6(..) => {
                            error!(target: "routing", "Expected IPV4 and not IPV6 address");
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
                        SocketAddr::V6(..) => {
                            error!(target: "probing", "Expected IPV4 and not IPV6 address");
                            events = vec![];
                        }
                    },
                    LocalContact(contact) => {
                        debug!(target: "probing", "Received contact info: {:#?}", contact);
                        debug!(target: &contact.wg_ip.to_string(), "Received local contacts");
                        events = network_manager.process_local_contact(contact);
                    }
                }
                for evt in events {
                    tx.send(evt).unwrap();
                }
            }
            Ok(Event::SendAdvertisement {
                to: destination,
                wg_ip,
            }) => {
                debug!(target: &wg_ip.to_string(),"Send advertisement");
                let routedb_version = network_manager.db_version();
                let opt_dp: Option<&DynamicPeer> = network_manager.dynamic_peer_for(&wg_ip);
                let advertisement =
                    UdpPacket::advertisement_from_config(static_config, routedb_version, opt_dp);
                let buf = serde_json::to_vec(&advertisement).unwrap();
                info!(target: "advertisement", "Send advertisement to {}", destination);
                crypt_socket.send_to(&buf, destination).ok();
            }
            Ok(Event::SendRouteDatabaseRequest { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send route database request");
                let request = UdpPacket::route_database_request();
                let buf = serde_json::to_vec(&request).unwrap();
                info!(target: "routing", "Send RouteDatabaseRequest to {}", destination);
                crypt_socket.send_to(&buf, SocketAddr::V4(destination)).ok();
            }
            Ok(Event::SendRouteDatabase { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send route database");
                let packages = network_manager.provide_route_database();
                for p in packages {
                    let buf = serde_json::to_vec(&p).unwrap();
                    info!(target: "routing", "Send RouteDatabase to {}", destination);
                    crypt_socket.send_to(&buf, SocketAddr::V4(destination)).ok();
                }
            }
            Ok(Event::SendLocalContactRequest { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send local contact request");
                let request = UdpPacket::local_contact_request();
                let buf = serde_json::to_vec(&request).unwrap();
                info!(target: "probing", "Send LocalContactRequest to {}", destination);
                crypt_socket.send_to(&buf, SocketAddr::V4(destination)).ok();
            }
            Ok(Event::SendLocalContact { to: destination }) => {
                debug!(target: &destination.ip().to_string(), "Send local contacts");
                let local_contact = UdpPacket::local_contact_from_config(
                    static_config,
                    network_manager.my_visible_admin_endpoint,
                    network_manager.my_visible_wg_endpoint,
                );
                trace!(target: "probing", "local contact to {:#?}", local_contact);
                let buf = serde_json::to_vec(&local_contact).unwrap();
                info!(target: "probing", "Send local contact to {}", destination);
                crypt_socket.send_to(&buf, SocketAddr::V4(destination)).ok();
            }
            Ok(Event::CheckAndRemoveDeadDynamicPeers) => {
                network_manager.output();
                let dead_peers = network_manager.check_timeouts(120);
                if !dead_peers.is_empty() {
                    info!(target: "dead_peer", "Dead peers found {}", dead_peers.len());
                }
                if !dead_peers.is_empty() {
                    for wg_ip in dead_peers {
                        debug!(target: &wg_ip.to_string(), "is dead => remove");
                        debug!(target: "dead_peer", "Found dead peer {}", wg_ip);
                        network_manager.remove_dynamic_peer(&wg_ip);
                    }
                    tx.send(Event::UpdateWireguardConfiguration).unwrap();
                    tx.send(Event::UpdateRoutes).unwrap();
                }
            }
            Ok(Event::UpdateWireguardConfiguration) => {
                info!("Update peers");
                let conf = static_config.as_conf_as_peer(&network_manager);
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
                        AddRoute {
                            to,
                            gateway: Some(gateway),
                        } => {
                            debug!(target: &to.to_string(), "add route with gateway {:?}", gateway);
                            wg_dev.add_route(&format!("{}/32", to), Some(gateway))?;
                        }
                        AddRoute { to, gateway: None } => {
                            debug!(target: &to.to_string(), "add route");
                            wg_dev.add_route(&format!("{}/32", to), None)?;
                        }
                        DelRoute {
                            to,
                            gateway: Some(gateway),
                        } => {
                            debug!(target: &to.to_string(), "del route with gateway {:?}", gateway);
                            wg_dev.del_route(&format!("{}/32", to), Some(gateway))?;
                        }
                        DelRoute { to, gateway: None } => {
                            debug!(target: &to.to_string(), "del route");
                            wg_dev.del_route(&format!("{}/32", to), None)?;
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
