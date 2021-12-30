use std::fs::File;
use std::io::Read;

use clap::{App, Arg};
use yaml_rust::YamlLoader;

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
            Arg::with_name("peer")
                .short("i")
                .long("ip")
                .help("IP of a peer to connect to"),
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

    let config = matches.value_of("config").unwrap_or("network.yaml");

    let mut file = File::open(config)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let conf = YamlLoader::load_from_str(&content);

    println!("{:?}", conf);

    Ok(())
}
