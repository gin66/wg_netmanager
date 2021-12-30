use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::{thread, time};

use clap::{App, Arg};
use yaml_rust::YamlLoader;

mod configuration;
mod unconnected;

use configuration::*;

fn main() -> Result<(), std::io::Error> {
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
    let private_key = &network["privateKey"].as_str().unwrap();
    if verbosity.all() {
        println!("Network private key from config file: {}", private_key);
    }

    let mut cmd = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    write!(cmd.stdin.as_ref().unwrap(), "{}", private_key)?;

    cmd.wait()?;

    let mut out = String::new();
    cmd.stdout.unwrap().read_to_string(&mut out)?;
    if verbosity.info() {
        println!("Network public key: {}", out);
    }

    let polling_interval = time::Duration::from_millis(1000);
    let static_config = StaticConfiguration::new(verbosity, "wg0", "10.1.1.1");
    let mut dynamic_config = DynamicConfiguration::WithoutDevice;
    loop {
        println!("Main loop");
        thread::sleep(polling_interval);
        use DynamicConfiguration::*;
        dynamic_config = match dynamic_config {
            WithoutDevice => unconnected::bring_up_device(&static_config),
            Unconfigured => unconnected::initial_connect(&static_config),
            _ => Unconfigured,
        }
    }
}
