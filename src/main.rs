use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;

use clap::{App, Arg};
use ctrlc;
use yaml_rust::YamlLoader;

use wg_netmanager::configuration::*;
use wg_netmanager::wg_dev::*;

enum Event {
    Udp(UdpPacket, SocketAddr),
    PeerListChange,
    CtrlC,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let private_key_listener = &network["privateKeyListener"].as_str().unwrap();
    let private_key_new_participant = &network["privateKeyNewParticipant"].as_str().unwrap();
    if verbosity.all() {
        println!(
            "Network private key from config file listener: {}",
            private_key_listener
        );
        println!(
            "Network private key from config file new participant: {}",
            private_key_new_participant
        );
    }
    let new_participant_ip = &network["newParticipant"].as_str().unwrap();
    let new_participant_listener_ip = &network["newParticipantListener"].as_str().unwrap();

    let mut peers: Vec<PublicPeer> = vec![];
    for p in conf[0]["peers"].as_vec().unwrap() {
        println!("PEER: {:?}", p);
        let public_ip = p["publicIp"].as_str().unwrap().to_string();
        let join_port = p["wgJoinPort"].as_i64().unwrap() as u16;
        let comm_port = p["wgPort"].as_i64().unwrap() as u16;
        let udp_port = p["udpPort"].as_i64().unwrap() as u16;
        let wg_ip = p["wgIp"].as_str().unwrap().to_string();
        let pp = PublicPeer {
            public_ip,
            join_port,
            comm_port,
            udp_port,
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

    let mut cmd = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    write!(cmd.stdin.as_ref().unwrap(), "{}", private_key_listener)?;
    cmd.wait()?;
    let mut public_key = String::new();
    cmd.stdout.unwrap().read_to_string(&mut public_key)?;
    let public_key_listener = public_key.trim();
    if verbosity.info() {
        println!("Network public key listener: {}", public_key_listener);
    }

    let mut cmd = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    write!(
        cmd.stdin.as_ref().unwrap(),
        "{}",
        private_key_new_participant
    )?;
    cmd.wait()?;
    let mut public_key = String::new();
    cmd.stdout.unwrap().read_to_string(&mut public_key)?;
    let public_key_new_participant = public_key.trim();
    if verbosity.info() {
        println!(
            "Network public key new participant: {}",
            public_key_new_participant
        );
    }

    let static_config = StaticConfiguration::new()
        .verbosity(verbosity)
        .name(computer_name)
        .wg_ip(wg_ip)
        .wg_name(interface)
        .new_participant_ip(*new_participant_ip)
        .new_participant_listener_ip(*new_participant_listener_ip)
        .my_public_key(my_public_key)
        .my_private_key(my_private_key)
        .my_public_key(my_public_key)
        .public_key_listener(public_key_listener)
        .public_key_new_participant(public_key_new_participant)
        .private_key_listener(*private_key_listener)
        .private_key_new_participant(*private_key_new_participant)
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

    // Bind to 0.0.0.0 so that udp from both wg interfaces can be received
    let port = static_config.my_udp_port().unwrap_or(0);
    println!("bind to 0.0.0.0:{}", port);
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;

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

    if static_config.is_listener() {
        loop_listener(static_config, socket, tx, rx)
    } else {
        loop_client(static_config, socket, tx, rx)
    }
}

fn loop_client(
    static_config: StaticConfiguration,
    socket: UdpSocket,
    tx: Sender<Event>,
    rx: Receiver<Event>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wg_dev = WireguardDeviceLinux::init(&static_config.wg_name, static_config.verbosity);

    let mut dynamic_config = DynamicConfigurationClient::WithoutDevice;
    let mut dynamic_peers = DynamicPeerList::default();
    let polling_interval = time::Duration::from_millis(1000);
    loop {
        println!("Main loop client");
        match rx.recv_timeout(polling_interval) {
            Ok(Event::CtrlC) => {
                break;
            }
            Err(_) => {
                // any timeout comes here
                use DynamicConfigurationClient::*;
                dynamic_config = match dynamic_config {
                    WithoutDevice => {
                        wg_dev.bring_up_device()?;
                        wg_dev.set_ip(&static_config.new_participant_ip)?;
                        let route = format!("{}/32", static_config.new_participant_listener_ip);
                        wg_dev.add_route(&route)?;
                        Unconfigured { peer_index: 0 }
                    }
                    Unconfigured { peer_index } => {
                        let conf = static_config.as_conf_for_new_participant(peer_index);
                        if static_config.verbosity.all() {
                            println!("Configuration for join ({}):\n{}\n", peer_index, conf);
                        }
                        wg_dev.set_conf(&conf)?;
                        ConfiguredForJoin { peer_index }
                    }
                    ConfiguredForJoin { peer_index } => {
                        let advertisement = UdpPacket::advertisement_from_config(&static_config);
                        let buf = serde_json::to_vec(&advertisement).unwrap();
                        let destination = format!(
                            "{}:{}",
                            static_config.new_participant_listener_ip,
                            static_config.udp_port(peer_index)
                        );
                        println!(
                            "Send advertisement to listener {} {}",
                            peer_index, destination
                        );
                        socket.send_to(&buf, destination).ok();
                        WaitForAdvertisement { peer_index, cnt: 0 }
                    }
                    WaitForAdvertisement { peer_index, cnt } => {
                        if cnt >= 5 {
                            // timeout, so try next peer
                            let new_peer_index = peer_index + 1;
                            if new_peer_index == static_config.peer_cnt {
                                wg_dev.take_down_device()?;
                                WithoutDevice
                            } else {
                                Unconfigured {
                                    peer_index: new_peer_index,
                                }
                            }
                        } else {
                            WaitForAdvertisement {
                                peer_index,
                                cnt: cnt + 1,
                            }
                        }
                    }
                    Connected => {
                        // any timeout comes here
                        dynamic_peers.output();
                        if dynamic_peers.peer.is_empty() {
                            wg_dev.take_down_device()?;
                            WithoutDevice
                        } else {
                            let dead_peers = dynamic_peers.check_timeouts();
                            if !dead_peers.is_empty() {
                                for wg_ip in dead_peers {
                                    println!("Found dead peer {}", wg_ip);
                                    dynamic_peers.remove_peer(&wg_ip);
                                    wg_dev.del_route(&format!("{}/32", wg_ip))?;
                                }
                                tx.send(Event::PeerListChange).unwrap();
                            }
                            let ping_peers = dynamic_peers.check_ping_timeouts();
                            for (wg_ip, udp_port) in ping_peers {
                                println!("Found ping peer {}...send ping", wg_ip);
                                let ping =
                                    UdpPacket::ping_from_config(&static_config);
                                let buf = serde_json::to_vec(&ping).unwrap();
                                let destination = format!("{}:{}", wg_ip, udp_port);
                                socket.send_to(&buf, destination).ok();
                            }
                            Connected
                        }
                    }
                }
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                use DynamicConfigurationClient::*;
                dynamic_config = match dynamic_config {
                    WithoutDevice => WithoutDevice,
                    Unconfigured { peer_index } => Unconfigured { peer_index },
                    ConfiguredForJoin { peer_index } => ConfiguredForJoin { peer_index },
                    WaitForAdvertisement { .. } => {
                        wg_dev.take_down_device()?;
                        dynamic_peers.add_peer(udp_packet, src_addr.port());
                        let conf = static_config.as_conf_as_peer(Some(&dynamic_peers));
                        if static_config.verbosity.all() {
                            println!("Configuration as peer\n{}\n", conf);
                        }
                        wg_dev.bring_up_device()?;
                        wg_dev.set_ip(&static_config.wg_ip)?;
                        wg_dev.set_conf(&conf)?;
                        for (wg_ip, _) in dynamic_peers.peer.iter() {
                            wg_dev.add_route(&format!("{}/32", wg_ip))?;
                        }
                        Connected
                    }
                    Connected => {
                        use UdpPacket::*;
                        match udp_packet {
                            ListenerAdvertisement { .. } | ClientAdvertisement { .. } => {
                                println!("Do not send advertisement to new participant");
                                if let Some(new_wg_ip) =
                                    dynamic_peers.add_peer(udp_packet, src_addr.port())
                                {
                                    tx.send(Event::PeerListChange).unwrap();
                                    wg_dev.add_route(&format!("{}/32", new_wg_ip))?;
                                }
                            }
                            ListenerPing { .. } | ClientPing {..} => {
                                dynamic_peers.update_peer(udp_packet, src_addr.port());
                            }
                        }
                        Connected
                    }
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

    wg_dev.take_down_device()?;
    Ok(())
}

fn loop_listener(
    static_config: StaticConfiguration,
    socket: UdpSocket,
    tx: Sender<Event>,
    rx: Receiver<Event>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wg_dev = WireguardDeviceLinux::init(&static_config.wg_name, static_config.verbosity);
    let wg_dev_listener = WireguardDeviceLinux::init("wg_listener", static_config.verbosity);

    wg_dev.bring_up_device()?;
    wg_dev.set_ip(&static_config.wg_ip)?;
    wg_dev_listener.bring_up_device()?;
    wg_dev_listener.set_ip(&static_config.new_participant_listener_ip)?;
    let route = format!("{}/32", static_config.new_participant_ip);
    wg_dev_listener.add_route(&route)?;

    let conf = static_config.as_conf_for_listener();
    if static_config.verbosity.all() {
        println!("Configuration for join:\n{}\n", conf);
    }
    wg_dev_listener.set_conf(&conf)?;

    let conf = static_config.as_conf_as_peer(None);
    if static_config.verbosity.all() {
        println!("Configuration as peer\n{}\n", conf);
    }
    wg_dev.set_conf(&conf)?;

    let mut dynamic_peers = DynamicPeerList::default();

    // let mut static_peer_index = 0;
    let polling_interval = time::Duration::from_millis(10000);
    loop {
        println!("Main loop listener: {} peers", dynamic_peers.peer.len());
        match rx.recv_timeout(polling_interval) {
            Ok(Event::CtrlC) => {
                break;
            }
            Err(_) => {
                // any timeout comes here
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

                let ping_peers = dynamic_peers.check_ping_timeouts();
                for (wg_ip, udp_port) in ping_peers {
                    println!("Found ping peer {}...send ping", wg_ip);
                    let ping =
                        UdpPacket::ping_from_config(&static_config);
                    let buf = serde_json::to_vec(&ping).unwrap();
                    let destination = format!("{}:{}", wg_ip, udp_port);
                    socket.send_to(&buf, destination).ok();
                }

                // TODO: Send Advertisement to next static peer
                //if static_config.peer_cnt > 0 {
                //    let conf = static_config.as_conf_for_new_participant(static_peer_index);
                //    if static_config.verbosity.all() {
                //        println!("Configuration for join ({}):\n{}\n", static_peer_index, conf);
                //    }
                //    wg_dev_listener.set_conf(&conf)?;
                //
                //    let advertisement = UdpPacket::advertisement_from_config(&static_config);
                //    let buf = serde_json::to_vec(&advertisement).unwrap();
                //    let destination = format!(
                //        "{}:{}",
                //        static_config.new_participant_listener_ip,
                //        static_config.udp_port(static_peer_index)
                //    );
                //    println!(
                //        "Send advertisement to listener {} {}",
                //        static_peer_index, destination
                //    );
                //    //socket.send_to(&buf, destination).ok();
                //}
            }
            Ok(Event::Udp(udp_packet, src_addr)) => {
                use UdpPacket::*;
                match udp_packet {
                    ListenerAdvertisement { .. } | ClientAdvertisement { .. } => {
                        println!("Send advertisement to new participant");
                        let advertisement = UdpPacket::advertisement_from_config(&static_config);
                        let buf = serde_json::to_vec(&advertisement).unwrap();
                        socket.send_to(&buf, src_addr).ok();

                        if let Some(new_wg_ip) = dynamic_peers.add_peer(udp_packet, src_addr.port())
                        {
                            tx.send(Event::PeerListChange).unwrap();
                            wg_dev.add_route(&format!("{}/32", new_wg_ip))?;
                        }
                    }
                    ListenerPing { .. } | ClientPing {..} => {
                        dynamic_peers.update_peer(udp_packet, src_addr.port());
                    }
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

    wg_dev_listener.take_down_device()?;
    wg_dev.take_down_device()?;
    Ok(())
}
