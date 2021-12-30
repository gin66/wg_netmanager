use std::process::{Command, Stdio};

use crate::configuration::*;

// ip link add wg0 type wireguard

pub fn check_device(config: &StaticConfiguration) -> bool {
    if config.verbosity.info() {
        println!("Check for device");
    }

    let status = Command::new("ip")
        .arg("link")
        .arg("show")
        .arg(&config.wg_name)
        .status()
        .unwrap();

    status.success()
}

pub fn bring_up_device(config: &StaticConfiguration) -> DynamicConfiguration {
    if config.verbosity.info() {
        println!("Bring up device");
    }

    let status = Command::new("sudo")
        .arg("ip")
        .arg("link")
        .arg("add")
        .arg(&config.wg_name)
        .arg("type")
        .arg("wireguard")
        .status()
        .unwrap();

    if status.success() {
        println!("Interface {} created", config.wg_name);
    }
    else {
    }

    DynamicConfiguration::Unconfigured 
}

pub fn take_down_device(config: &StaticConfiguration) -> DynamicConfiguration {
    if config.verbosity.info() {
        println!("Take down device");
    }

    let status = Command::new("sudo")
        .arg("ip")
        .arg("link")
        .arg("del")
        .arg(&config.wg_name)
        .status()
        .unwrap();

    if status.success() {
        println!("Interface {} created", config.wg_name);
    }
    else {
    }

    DynamicConfiguration::Unconfigured 
}

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
    println!("{:?}",output);

    DynamicConfiguration::Unconfigured
}
