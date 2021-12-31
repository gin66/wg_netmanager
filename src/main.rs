use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::{thread, time};
use std::sync::mpsc::channel;

use ctrlc;
use clap::{App, Arg};
use yaml_rust::YamlLoader;

use wg_netmanager::configuration::*;
use wg_netmanager::unconnected::*;
use wg_netmanager::wg_dev::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = channel();
        
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
                    .expect("Error setting Ctrl-C handler");

    let matches = App::new("Wireguard Network Manager")
        .version("0.1")
        .author("Jochen Kiemes <jochen@kiemes.de>")
        .about("Manages a network of wireguard nodes with no central server.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Custom config file in ini-style")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("listen_port")
                .short("l")
                .long("listen")
                .help("Static listen port"),
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
        .get_matches();

    let verbosity = match matches.occurrences_of("v") {
        0 => Verbosity::Silent,
        1 => Verbosity::Info,
        2 | _ => Verbosity::All,
    };

    let interface = matches.value_of("interface").unwrap();

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
        println!("Network private key from config file listener: {}", private_key_listener);
        println!("Network private key from config file new participant: {}", private_key_new_participant);
    }
    let new_participant_ip = &network["newParticipant"].as_str().unwrap();
    let new_participant_listener_ip = &network["newParticipantListener"].as_str().unwrap();

    let mut peers: Vec<PublicPeer> = vec![];
    for p in conf[0]["peers"].as_vec() {
        println!("{:?}",p);
        let public_ip = p[0]["publicIp"].as_str().unwrap().to_string();
        let join_port = p[0]["wgJoinPort"].as_i64().unwrap() as u16;
        let comm_port = p[0]["wgPort"].as_i64().unwrap() as u16;
        let wg_ip = p[0]["wgIp"].as_str().unwrap().to_string();
        let pp = PublicPeer {
            public_ip,
            join_port,
            comm_port,
            wg_ip,
        };
        peers.push(pp);
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
    write!(cmd.stdin.as_ref().unwrap(), "{}", private_key_new_participant)?;
    cmd.wait()?;
    let mut public_key = String::new();
    cmd.stdout.unwrap().read_to_string(&mut public_key)?;
    let public_key_new_participant = public_key.trim();
    if verbosity.info() {
        println!("Network public key new participant: {}", public_key_new_participant);
    }


    let polling_interval = time::Duration::from_millis(1000);
    let static_config = StaticConfiguration::new()
        .verbosity(verbosity)
        .wg_name(interface)
        .unconnected_ip("10.1.1.1")
        .new_participant_ip(*new_participant_ip)
        .new_participant_listener_ip(*new_participant_listener_ip)
        .public_key_listener(public_key_listener)
        .public_key_new_participant(public_key_new_participant)
        .private_key_listener(*private_key_listener)
        .private_key_new_participant(*private_key_new_participant)
        .peers(peers)
        .build();
    let wg_dev = WireguardDeviceLinux::init(&static_config);

    let mut dynamic_config = DynamicConfiguration::WithoutDevice;
    while rx.recv_timeout(polling_interval).is_err() {
        use DynamicConfiguration::*;
        println!("Main loop");
        dynamic_config = match dynamic_config {
            WithoutDevice => {
                wg_dev.bring_up_device()?;
                Unconfigured
            }
            Unconfigured => {
                let conf = static_config.as_conf(0);
                if verbosity.all() {
                    println!("Configuration for join:\n{}\n", conf);
                }
                wg_dev.set_conf(&conf);
                ConfiguredForJoin
            },
            ConfiguredForJoin => ConfiguredForJoin,
            Connected => Connected,
            Disconnected => Disconnected,
        }
    }

    wg_dev.take_down_device();

    Ok(())
}
