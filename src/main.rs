use std::fs::File;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;
use std::time::SystemTime;

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

// ===================== Logging Set Up =====================
fn set_up_logging(log_filter: log::LevelFilter) {
    use fern::colors::*;
    // configure colors for the whole line
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::Blue)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);

    // configure colors for the name of the level.
    // since almost all of them are the same as the color for the whole line, we
    // just clone `colors_line` and overwrite our changes
    let colors_level = colors_line.info(Color::Green);
    // here we set up our fern Dispatch
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}{date} {level} {color_line}{message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%H:%M:%S"),
                //target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        // set the default log level. to filter out verbose log messages from dependencies, set
        // this to Warn and overwrite the log level for your crate.
        .level(log_filter)
        // change log levels for individual modules. Note: This looks for the record's target
        // field which defaults to the module path but can be overwritten with the `target`
        // parameter:
        // `info!(target="special_target", "This log message is about special_target");`
        //.level_for("pretty_colored", log::LevelFilter::Trace)
        // output to stdout
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    debug!("finished setting up logging! yay!");
}

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
            Arg::with_name("wireguard_port")
                .short("w")
                .long("wireguard-port")
                .value_name("PORT")
                .default_value("55555")
                .help("Wireguard udp port aka Listen port, if not defined in config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("admin_port")
                .short("u")
                .long("admin-port")
                .value_name("PORT")
                .default_value("50000")
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

    let log_filter = match matches.occurrences_of("v") {
        0 => log::LevelFilter::Error,
        1 => log::LevelFilter::Warn,
        2 => log::LevelFilter::Info,
        3 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    // Select logger based on command line flag
    //
    let use_tui = matches.is_present("tui");

    if use_tui {
        tui_logger::init_logger(log::LevelFilter::Trace).unwrap();
        tui_logger::set_default_level(log::LevelFilter::Trace);
    } else {
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

    let mut peers: Vec<PublicPeer> = vec![];
    for p in conf[0]["peers"].as_vec().unwrap() {
        info!("STATIC PEER: {:#?}", p);
        let public_ip: IpAddr = p["publicIp"].as_str().unwrap().parse().unwrap();
        let wg_port = p["wgPort"].as_i64().unwrap() as u16;
        let admin_port = p["adminPort"].as_i64().unwrap() as u16;
        let wg_ip: Ipv4Addr = p["wgIp"].as_str().unwrap().parse().unwrap();
        let pp = PublicPeer {
            public_ip,
            wg_port,
            admin_port,
            wg_ip,
        };
        peers.push(pp);
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
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
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
        .build();

    run(static_config)
}

fn run(static_config: StaticConfiguration) -> BoxResult<()> {
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

    let wg_dev = WireguardDeviceLinux::init(&static_config.wg_name);
    // in case there are dangling devices
    wg_dev.take_down_device().ok();

    wg_dev.bring_up_device()?;
    wg_dev.set_ip(&static_config.wg_ip)?;

    let mut tui_app = if static_config.use_tui {
        TuiApp::init(tx.clone())?
    } else {
        TuiApp::off()
    };

    let rc = main_loop(static_config, &wg_dev, crypt_socket, tx, rx, &mut tui_app);

    wg_dev.take_down_device().ok();

    tui_app.deinit()?;

    rc
}

fn main_loop(
    static_config: StaticConfiguration,
    wg_dev: &dyn WireguardDevice,
    crypt_socket: CryptUdp,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    tui_app: &mut TuiApp,
) -> BoxResult<()> {
    let mut network_manager = NetworkManager::new(static_config.wg_ip);

    // set up initial wireguard configuration without peers
    tx.send(Event::PeerListChange).unwrap();
    tx.send(Event::SendAdvertisementToPublicPeers).unwrap();

    let mut tick_cnt = 0;
    loop {
        trace!("Main loop: {} peers", network_manager.peer.len());
        let evt = rx.recv();
        trace!("{:?}", evt);
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
                    info!("Main loop: {} peers", network_manager.peer.len());
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
                    tx.send(Event::SendAdvertisement { to: destination })
                        .unwrap();
                }
            }
            Ok(Event::SendAdvertisementToPublicPeers) => {
                // These advertisements are sent to the known internet address as defined in the config file.
                // As all udp packets are encrypted, this should not be an issue.
                //
                for peer in static_config.peers.iter() {
                    if !network_manager.knows_peer(&peer.wg_ip) {
                        // ensure not to send to myself
                        if peer.wg_ip != static_config.wg_ip {
                            let destination = SocketAddr::new(peer.public_ip, peer.admin_port);
                            tx.send(Event::SendAdvertisement { to: destination })
                                .unwrap();
                        }
                    }
                }
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                use UdpPacket::*;
                match udp_packet {
                    Advertisement(ad) => {
                        if let Some(new_wg_ip) =
                            network_manager.analyze_advertisement_for_new_peer(&ad, src_addr.port())
                        {
                            info!("Unknown peer {}", src_addr);
                            network_manager.add_dynamic_peer(&new_wg_ip);

                            tx.send(Event::PeerListChange).unwrap();
                            tx.send(Event::UpdateRoutes).unwrap();

                            // Answers to advertisments are only sent, if the wireguard ip is not
                            // in the list of dynamic peers and as such is new.
                            // Consequently the reply is sent over the internet and not via
                            // wireguard tunnel.
                            tx.send(Event::SendAdvertisement { to: src_addr }).unwrap();
                        } else {
                            info!("Existing peer {}", src_addr);
                        }
                        if let Some(wg_ip) = network_manager.analyze_advertisement(&ad) {
                            // need to request new route database
                            let destination = SocketAddrV4::new(wg_ip, src_addr.port());
                            tx.send(Event::SendRouteDatabaseRequest { to: destination })
                                .unwrap();
                        }
                    }
                    RouteDatabaseRequest => {
                        info!("RouteDatabaseRequest from {:?}", src_addr);
                        match src_addr {
                            SocketAddr::V4(destination) => {
                                tx.send(Event::SendRouteDatabase { to: destination })
                                    .unwrap();
                            }
                            SocketAddr::V6(..) => {
                                error!("Expected IPV4 and not IPV6 address");
                            }
                        }
                    }
                    RouteDatabase(req) => {
                        info!("RouteDatabase from {}", src_addr);
                        if network_manager.process_route_database(req) {
                            tx.send(Event::UpdateRoutes).unwrap();
                        }
                    }
                }
            }
            Ok(Event::SendAdvertisement { to: destination }) => {
                let routedb_version = network_manager.db_version();
                let advertisement =
                    UdpPacket::advertisement_from_config(&static_config, routedb_version);
                let buf = serde_json::to_vec(&advertisement).unwrap();
                info!("Send advertisement to {}", destination);
                crypt_socket.send_to(&buf, destination).ok();
            }
            Ok(Event::SendRouteDatabaseRequest { to: destination }) => {
                let request = UdpPacket::route_database_request();
                let buf = serde_json::to_vec(&request).unwrap();
                info!("Send RouteDatabaseRequest to {}", destination);
                crypt_socket.send_to(&buf, destination).ok();
            }
            Ok(Event::SendRouteDatabase { to: destination }) => {
                let packages = network_manager.provide_route_database();
                for p in packages {
                    let buf = serde_json::to_vec(&p).unwrap();
                    info!("Send RouteDatabase to {}", destination);
                    crypt_socket.send_to(&buf, destination).ok();
                }
            }
            Ok(Event::CheckAndRemoveDeadDynamicPeers) => {
                network_manager.output();
                let dead_peers = network_manager.check_timeouts(120);
                if !dead_peers.is_empty() {
                    for wg_ip in dead_peers {
                        info!("Found dead peer {}", wg_ip);
                        network_manager.remove_dynamic_peer(&wg_ip);
                    }
                    tx.send(Event::PeerListChange).unwrap();
                    tx.send(Event::UpdateRoutes).unwrap();
                }
            }
            Ok(Event::PeerListChange) => {
                info!("Update peers");
                let conf = static_config.as_conf_as_peer(&network_manager);
                info!("Configuration as peer\n{}\n", conf);
                wg_dev.sync_conf(&conf)?;
            }
            Ok(Event::UpdateRoutes) => {
                let changes = network_manager.get_route_changes();
                for rc in changes {
                    use RouteChange::*;
                    debug!("{:?}", rc);
                    match rc {
                        AddRouteWithGateway { to, gateway } => {
                            wg_dev.add_route(&format!("{}/32", to), Some(gateway))?;
                        }
                        AddRoute { to } => {
                            wg_dev.add_route(&format!("{}/32", to), None)?;
                        }
                        DelRouteWithGateway { to, gateway } => {
                            wg_dev.del_route(&format!("{}/32", to), Some(gateway))?;
                        }
                        DelRoute { to } => {
                            wg_dev.del_route(&format!("{}/32", to), None)?;
                        }
                    }
                }
                tx.send(Event::PeerListChange).unwrap();
            }
            Ok(Event::TuiApp(evt)) => {
                tui_app.process_event(evt);
                tui_app.draw()?;
            }
        }
    }
    Ok(())
}
