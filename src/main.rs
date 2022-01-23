use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};
use log::*;
use yaml_rust::{Yaml, YamlLoader};

use wg_netmanager::configuration::*;
use wg_netmanager::error::*;
use wg_netmanager::*;

fn get_option_bool(matches: &ArgMatches, config: &Option<Yaml>, option_name: &'static str) -> bool {
    if matches.is_present(option_name) {
        return true;
    }

    if let Some(conf) = config.as_ref() {
        if let Some(val) = conf[option_name].as_bool() {
            return val;
        }
    }

    false
}
fn get_option_string(
    matches: &ArgMatches,
    config: &Option<Yaml>,
    option_name: &'static str,
) -> BoxResult<String> {
    if let Some(val) = matches.value_of(option_name) {
        return Ok(val.to_string());
    } else if let Some(conf) = config.as_ref() {
        if let Some(val) = conf[option_name].as_str() {
            return Ok(val.to_string());
        }
    }
    Err(format!("Configuration option <{}> is not defined", option_name).into())
}

fn main() -> BoxResult<()> {
    let matches = App::new("Wireguard Network Manager")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jochen Kiemes <jochen@kiemes.de>")
        .about("Manages a network of wireguard nodes with no central server.")
        .arg(
            Arg::with_name("network_config")
                .short("c")
                .long("network_config")
                .default_value(Arch::default_path_to_network_yaml())
                .value_name("NETWORK")
                .help("Network config file in yaml-style")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("peer_config")
                .short("p")
                .long("peer_config")
                .default_value(Arch::default_path_to_peer_yaml())
                .value_name("PEER")
                .help("Peer config file in yaml-style")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("existingInterface")
                .short("e")
                .long("existing-wg")
                .help("Use an existing wireguard interface and do not try to create one"),
        )
        .arg(
            Arg::with_name("wireguard_port")
                .short("w")
                .long("wireguard-port")
                .value_name("PORT")
                .help("Wireguard udp port aka Listen port, if not defined in config file as part of endpoint")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("admin_port")
                .short("u")
                .long("admin-port")
                .value_name("PORT")
                .help("udp port for encrypted communication")
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
            Arg::with_name("wgInterface")
                .short("i")
                .long("wireguard-interface")
                .help("Sets the wireguard interface")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("wgIp")
                .short("a")
                .long("wireguard-address")
                .help("Sets the wireguard ip address (ipv4)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .help("Sets the name for this computer")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("Output")
                .short("O")
                .help("Output the static configuration and exit immediately (for test only)"),
        )
        .subcommand(App::new("install").about("Support installation as deamon"))
        .get_matches();

    let use_tui = matches.is_present("tui");

    let mut opt_peer_conf: Option<Yaml> = None;
    // unwrap() is ok here due to the default value in clap
    let peer_config = matches.value_of("peer_config").unwrap();
    match File::open(peer_config) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            let mut peer_conf_vec = YamlLoader::load_from_str(&content).unwrap();
            if peer_conf_vec.len() != 1 {
                return Err("Malformed peer configuration".into());
            }
            opt_peer_conf = Some(peer_conf_vec.remove(0));
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                return Err(format!("Permission denied for {}", peer_config).into());
            }
            std::io::ErrorKind::NotFound => {
                // That's OK
            }
            _ => return Err(Box::new(e)),
        },
    }

    let computer_name = get_option_string(&matches, &opt_peer_conf, "name")?;

    // Select logger based on command line flag
    //
    // Cannot initialize earlier, because the computer name is needed
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

    let network_config = matches.value_of("network_config").unwrap();
    let network_conf: Yaml;
    match File::open(network_config) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            let mut network_conf_vec = YamlLoader::load_from_str(&content).unwrap();
            if network_conf_vec.len() != 1 {
                return Err("Malformed network configuration".into());
            }
            network_conf = network_conf_vec.remove(0);
            debug!("Raw configuration:");
            debug!("{:#?}", network_conf);
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                return Err(format!("Permission denied for {}", network_config).into());
            }
            std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "Cannot find required network configuration: {}",
                    network_config
                )
                .into());
            }
            _ => return Err(Box::new(e)),
        },
    }

    let ip_list = Arch::get_local_interfaces();

    let use_existing_interface = get_option_bool(&matches, &opt_peer_conf, "existingInterface");
    let interface = get_option_string(&matches, &opt_peer_conf, "wgInterface")?;
    let wg_ip_string = get_option_string(&matches, &opt_peer_conf, "wgIp")?;
    let wg_ip: Ipv4Addr = wg_ip_string.parse()?;

    // Due to default values in clap, the unwraps() before parse() are ok
    let last = *(wg_ip.octets().last().unwrap()) as usize;
    let wg_port: u16 = matches.value_of("wireguard_port").unwrap_or(&format!("{}",50000+last)).parse()?;
    let admin_port: u16 = matches.value_of("admin_port").unwrap_or(&format!("{}",50500+last)).parse()?;

    let network = &network_conf["network"];
    let shared_key = base64::decode(
        &network["sharedKey"]
            .as_str()
            .ok_or("sharedKey is not defined or not a string")?,
    )?;
    let subnet: ipnet::Ipv4Net = network["subnet"]
        .as_str()
        .ok_or("subnet is not defined or not a string")?
        .parse()?;

    if !subnet.contains(&wg_ip) {
        return Err(format!("{} is outside of subnet {}", wg_ip, subnet).into());
    }

    let mut peers: HashMap<Ipv4Addr, PublicPeer> = HashMap::new();
    for p in network_conf["peers"]
        .as_vec()
        .ok_or("no peers defined in config file")?
    {
        info!("STATIC PEER: {:#?}", p);
        let endpoint = p["endPoint"]
            .as_str()
            .ok_or("no endpoint defined")?
            .to_string();
        let mut flds = endpoint.split(':').collect::<Vec<_>>();
        let port_str = flds.pop().ok_or("endpoint should be <hostname/ip:port>")?;
        let wg_port = (*port_str).parse::<u16>()?;
        let admin_port = p["adminPort"]
            .as_i64()
            .ok_or("Cannot parse adminPort as integer")? as u16;
        let wg_ip: Ipv4Addr = p["wgIp"]
            .as_str()
            .ok_or("wgIp not defined or not a string")?
            .parse()?;
        let pp = PublicPeer {
            endpoint,
            admin_port,
            wg_port,
            wg_ip,
        };
        peers.insert(wg_ip, pp);
    }

    let wg_dev = Arch::get_wg_dev(&interface);
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
        .network_yaml_filename(network_config)
        .peer_yaml_filename(peer_config)
        .build();

    let subcommand = matches.subcommand();
    if subcommand.0 == "install" {
        return Arch::command_install(subcommand.1.unwrap(), static_config);
    }

    if matches.is_present("Output") {
        println!("{:#?}", static_config);
        return Ok(());
    }

    wg_netmanager::run_loop::run(&static_config, wg_dev)
}
