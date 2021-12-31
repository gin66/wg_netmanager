use std::process::{Command, Stdio};

use crate::configuration::*;

pub fn initial_connect(config: &StaticConfiguration) -> DynamicConfiguration {
    if config.verbosity.info() {
        println!("Initialize unconnected wireguard interface");
    }

    let output = Command::new("sudo")
        .arg("ifconfig")
        .arg(&config.wg_name)
        .arg(&config.unconnected_ip)
        .output()
        .unwrap();
    println!("{:?}", output);

    DynamicConfiguration::Unconfigured
}
