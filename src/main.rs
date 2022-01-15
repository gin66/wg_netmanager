use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr};

use clap::{App, Arg};
use log::*;
use yaml_rust::YamlLoader;

use wg_netmanager::configuration::*;
use wg_netmanager::error::*;
use wg_netmanager::*;

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
            Arg::with_name("logfile")
                .short("l")
                .help("log to file <name>.log"),
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
    let computer_name = matches.value_of("name").unwrap();

    // Select logger based on command line flag
    //
    let opt_fname = if matches.is_present("logfile") {
        Some(format!("{}.log", computer_name))
    } else {
        None
    };
    if use_tui {
        tui_logger::init_logger(log::LevelFilter::Trace).unwrap();
        tui_logger::set_default_level(log::LevelFilter::Trace);
        if let Some(fname) = opt_fname {
            tui_logger::set_log_file(&fname)?;
        }
    } else {
        let log_filter = match matches.occurrences_of("v") {
            0 => log::LevelFilter::Error,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        set_up_logging(log_filter, opt_fname)?;
    }

    let interface = matches.value_of("interface").unwrap();
    let wg_ip: Ipv4Addr = matches.value_of("wg_ip").unwrap().parse().unwrap();
    let wg_port: u16 = matches.value_of("wireguard_port").unwrap().parse().unwrap();
    let admin_port: u16 = matches.value_of("admin_port").unwrap().parse().unwrap();

    let ip_list = Arch::get_local_interfaces();

    let config = matches.value_of("config").unwrap_or("network.yaml");

    let mut file = File::open(config)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let conf = YamlLoader::load_from_str(&content).unwrap();

    debug!("Raw configuration:");
    debug!("{:#?}", conf);

    let network = &conf[0]["network"];
    let shared_key = base64::decode(&network["sharedKey"].as_str().unwrap()).unwrap();
    let subnet: ipnet::Ipv4Net = network["subnet"].as_str().unwrap().parse().unwrap();

    if !subnet.contains(&wg_ip) {
        return Err(format!("{} is outside of {}", wg_ip, subnet).into());
    }

    let mut peers: HashMap<Ipv4Addr, PublicPeer> = HashMap::new();
    for p in conf[0]["peers"].as_vec().unwrap() {
        info!("STATIC PEER: {:#?}", p);
        let endpoint = p["endPoint"].as_str().unwrap().to_string();
        let mut flds = endpoint.split(':').collect::<Vec<_>>();
        let port_str = flds.pop().unwrap();
        let wg_port = (*port_str).parse::<u16>().unwrap();
        let admin_port = p["adminPort"].as_i64().unwrap() as u16;
        let wg_ip: Ipv4Addr = p["wgIp"].as_str().unwrap().parse().unwrap();
        let pp = PublicPeer {
            endpoint,
            admin_port,
            wg_port,
            wg_ip,
        };
        peers.insert(wg_ip, pp);
    }

    let wg_dev = Arch::get_wg_dev(interface);
    let (my_private_key, my_public_key) = wg_dev.create_key_pair()?;
    trace!("My private key: {}", my_private_key);
    trace!("My public key: {}", my_public_key);
    let timestamp = wg_netmanager::util::now();
    let my_public_key_with_time = PublicKeyWithTime {
        key: my_public_key,
        priv_key_creation_time: timestamp,
    };

    let static_config = StaticConfiguration::builder()
        .name(computer_name)
        .ip_list(ip_list)
        .wg_ip(wg_ip)
        .wg_name(interface)
        .wg_port(wg_port)
        .admin_port(admin_port)
        .subnet(subnet)
        .shared_key(shared_key)
        .my_public_key(my_public_key_with_time)
        .my_private_key(my_private_key)
        .peers(peers)
        .use_tui(use_tui)
        .use_existing_interface(use_existing_interface)
        .build();

    wg_netmanager::main_loop::run(&static_config, wg_dev)
}

