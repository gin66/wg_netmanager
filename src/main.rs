use std::fs::File;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;

use clap::{App, Arg};
use ctrlc;
use yaml_rust::YamlLoader;

use wg_netmanager::error::*;
use wg_netmanager::configuration::*;
use wg_netmanager::wg_dev::*;
use wg_netmanager::crypt_udp::CryptUdp;

enum Event {
    Udp(UdpPacket, SocketAddr),
    PeerListChange,
    CtrlC,
    SendAdvertsementToPublicPeers,
    SendPingToAllDynamicPeers,
    CheckAndRemoveDeadDynamicPeers,
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

    let verbosity = match matches.occurrences_of("v") {
        0 => Verbosity::Silent,
        1 => Verbosity::Info,
        2 | _ => Verbosity::All,
    };

    let interface = matches.value_of("interface").unwrap();
    let wg_ip = matches.value_of("wg_ip").unwrap();
    let computer_name = matches.value_of("name").unwrap();

    let config = matches.value_of("config").unwrap_or("network.yaml");

    let mut file = File::open(config)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let conf = YamlLoader::load_from_str(&content).unwrap();

    if verbosity.all() {
        println!("Raw configuration:");
        println!("{:?}", conf);
    }

    let network = &conf[0]["network"];
    let shared_key = base64::decode(&network["sharedKey"].as_str().unwrap()).unwrap();

    let mut peers: Vec<PublicPeer> = vec![];
    for p in conf[0]["peers"].as_vec().unwrap() {
        println!("PEER: {:?}", p);
        let public_ip = p["publicIp"].as_str().unwrap().to_string();
        let comm_port = p["wgPort"].as_i64().unwrap() as u16;
        let admin_port = p["adminPort"].as_i64().unwrap() as u16;
        let wg_ip = p["wgIp"].as_str().unwrap().to_string();
        let pp = PublicPeer {
            public_ip,
            comm_port,
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
    if verbosity.info() {
        println!("Network private key: {}", my_private_key);
    }
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
    if verbosity.info() {
        println!("Network public key: {}", my_public_key);
    }

    let static_config = StaticConfiguration::new()
        .verbosity(verbosity)
        .name(computer_name)
        .wg_ip(wg_ip)
        .wg_name(interface)
        .my_public_key(my_public_key)
        .my_private_key(my_private_key)
        .my_public_key(my_public_key)
        .peers(peers)
        .build();

    let (tx, rx) = channel();

    let tx_handler = tx.clone();
    ctrlc::set_handler(move || {
        println!("CTRL-C");
        tx_handler
            .send(Event::CtrlC)
            .expect("Could not send signal on channel.")
    })
    .expect("Error setting Ctrl-C handler");

    let port = static_config.my_admin_port().unwrap_or(0);
    println!("bind to 0.0.0.0:{}", port);
    let socket = CryptUdp::bind(port)?.key(&shared_key)?;

    // Set up udp receiver thread
    let tx_clone = tx.clone();
    let socket_clone = socket.try_clone().expect("couldn't clone the socket");
    std::thread::spawn(move || {
        loop {
            let mut buf = [0; 1000];
            match socket_clone.recv_from(&mut buf) {
                Ok((received, src_addr)) => {
                    println!("received {} bytes from {:?}", received, src_addr);
                    match serde_json::from_slice::<UdpPacket>(&buf[..received]) {
                        Ok(udp_packet) => {
                            tx_clone.send(Event::Udp(udp_packet, src_addr)).unwrap();
                        }
                        Err(e) => {
                            println!("Error in json decode: {:?}", e);
                        }
                    }
                }
                Err(_e) => {
                    //println!("{:?}",e);
                }
            }
        }
    });

    let wg_dev = WireguardDeviceLinux::init(&static_config.wg_name, static_config.verbosity);
    // in case there are dangling devices
    wg_dev.take_down_device().ok();

    wg_dev.bring_up_device()?;
    wg_dev.set_ip(&static_config.wg_ip)?;

    let rc = main_loop(static_config, &wg_dev, socket, tx, rx);

    wg_dev.take_down_device().ok();

    rc
}

fn main_loop(
    static_config: StaticConfiguration,
    wg_dev: &dyn WireguardDevice,
    socket: CryptUdp,
    tx: Sender<Event>,
    rx: Receiver<Event>,
) -> BoxResult<()> {
    let mut dynamic_peers = DynamicPeerList::default();

    // set up initial wireguard configuration without peers
    tx.send(Event::PeerListChange).unwrap();

    // The main difference between listener and client is, 
    // that listener is reachable.

    let polling_interval = time::Duration::from_millis(10000);

    let mut time_5s = time::Instant::now();
    let mut time_60s = time::Instant::now();
    tx.send(Event::SendAdvertsementToPublicPeers).unwrap();

    loop {
        println!("Main loop: {} peers", dynamic_peers.peer.len());
        match rx.recv_timeout(polling_interval) {
            Ok(Event::CtrlC) => {
                break;
            }
            Err(_) => {
                // any timeout comes here
                if time_60s.elapsed().as_secs() >= 60 {
                    time_60s = time::Instant::now();
                    tx.send(Event::SendAdvertsementToPublicPeers).unwrap();
                }
                if time_5s.elapsed().as_secs() >= 5 {
                    time_5s = time::Instant::now();
                    tx.send(Event::SendPingToAllDynamicPeers).unwrap();
                    tx.send(Event::CheckAndRemoveDeadDynamicPeers).unwrap();
                }
            }
            Ok(Event::SendPingToAllDynamicPeers) => {
                let ping_peers = dynamic_peers.check_ping_timeouts();
                for (wg_ip, udp_port) in ping_peers {
                    let ping = UdpPacket::ping_from_config(&static_config);
                    let buf = serde_json::to_vec(&ping).unwrap();
                    let destination = format!("{}:{}", wg_ip, udp_port);
                    println!("Found ping peer {}...send ping", destination);
                    socket.send_to(&buf, destination).ok();
                }
            }
            Ok(Event::SendAdvertsementToPublicPeers) => {
                for peer in static_config.peers.iter() {
                    if !dynamic_peers.knows_peer(&peer.wg_ip) {
                        let advertisement = UdpPacket::advertisement_from_config(&static_config);
                        let buf = serde_json::to_vec(&advertisement).unwrap();
                        let destination = 
                            format!(
                            "{}:{}",
                            peer.public_ip,
                            peer.admin_port
                        );
                        println!(
                            "Send advertisement to {}",
                            destination
                        );
                        socket.send_to(&buf, destination).ok();
                    }
                }
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                use UdpPacket::*;
                match udp_packet {
                    ListenerAdvertisement { .. }
                    | ClientAdvertisement { .. } => {
                        if let Some(new_wg_ip) = dynamic_peers.add_peer(udp_packet, src_addr.port())
                        {
                            tx.send(Event::PeerListChange).unwrap();
                            wg_dev.add_route(&format!("{}/32", new_wg_ip))?;

                            println!("Send advertisement to new participant");
                            let advertisement = UdpPacket::advertisement_from_config(&static_config);
                            let buf = serde_json::to_vec(&advertisement).unwrap();
                            socket.send_to(&buf, src_addr).ok();
                        }
                    }
                    ListenerPing { .. } | ClientPing {..} => {
                        dynamic_peers.update_peer(udp_packet, src_addr.port());
                    }
                }
            }
            Ok(Event::CheckAndRemoveDeadDynamicPeers) => {
                dynamic_peers.output();
                let dead_peers = dynamic_peers.check_timeouts();
                if !dead_peers.is_empty() {
                    for wg_ip in dead_peers {
                        println!("Found dead peer {}", wg_ip);
                        dynamic_peers.remove_peer(&wg_ip);
                        wg_dev.del_route(&format!("{}/32", wg_ip))?;
                    }
                    tx.send(Event::PeerListChange).unwrap();
                }
            }
            Ok(Event::PeerListChange) => {
                println!("Update peers");
                let conf = static_config.as_conf_as_peer(Some(&dynamic_peers));
                if static_config.verbosity.all() {
                    println!("Configuration as peer\n{}\n", conf);
                }
                wg_dev.sync_conf(&conf)?;
            }
        }
    }
    Ok(())
}
