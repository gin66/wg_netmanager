mod interfaces;
mod wg_dev_linuxkernel;

use std::net::IpAddr;
use std::sync::mpsc;

use clap::ArgMatches;
use simple_signal::{self, Signal};

use crate::arch_def::Architecture;
use crate::configuration::StaticConfiguration;
use crate::error::BoxResult;
use crate::event::Event;
use crate::wg_dev::WireguardDevice;

use wg_dev_linuxkernel::WireguardDeviceLinux;

pub struct ArchitectureLinux {}
impl Architecture for ArchitectureLinux {
    fn default_path_to_network_yaml() -> &'static str {
        "/etc/wg_netmanager/network.yaml"
    }
    fn default_path_to_peer_yaml() -> &'static str {
        "/etc/wg_netmanager/peer.yaml"
    }
    fn ipv4v6_socket_setup() -> (bool, bool, bool) {
        // for sysctl net.ipv6.bindv6only=0 systems like linux: ipv6 socket reads/sends ipv4 messages
        (false, false, true)
    }
    fn get_local_interfaces() -> Vec<IpAddr> {
        interfaces::get()
    }
    fn get_wg_dev<T: Into<String>>(wg_name: T) -> Box<dyn WireguardDevice> {
        Box::new(WireguardDeviceLinux::init(wg_name))
    }
    fn command_install(matches: &ArgMatches, static_config: StaticConfiguration) -> BoxResult<()> {
        let kill_candidates = [
            "/run/current-system/sw/bin/kill",
            "/bin/kill",
            "/usr/bin/kill",
        ];
        let kill_fname = kill_candidates
            .into_iter()
            .filter(|fname| std::path::Path::new(fname).exists())
            .collect::<Vec<_>>();

        let _ = matches.is_present("force");
        let mut lines: Vec<String> = vec![];
        lines.push(
            "Copy the following lines to /etc/systemd/system/wg_netmanager.service".to_string(),
        );
        lines.push("#================================".to_string());
        lines.push("[Unit]".to_string());
        lines.push("Description= The Wireguard network manager".to_string());
        lines.push(format!(
            "ConditionPathExists={}",
            static_config.network_yaml_filename
        ));
        if let Some(fname) = static_config.peer_yaml_filename.as_ref() {
            lines.push(format!("ConditionPathExists={}", fname));
        }
        lines.push("".to_string());
        lines.push("[Service]".to_string());
        lines.push("Type=simple ".to_string());
        lines.push(format!(
            "ExecStart={}",
            std::env::current_exe().unwrap().to_str().unwrap()
        ));
        lines.push(format!("ExecStop={} -HUP $MAINPID", kill_fname[0]));
        lines.push("".to_string());
        lines.push("[Install]".to_string());
        lines.push("WantedBy=multi-user.target".to_string());
        lines.push("#================================".to_string());
        lines.push("".to_string());
        lines.push("Then execute:".to_string());
        lines.push("    sudo systemctl daemon-reload".to_string());
        lines.push("    sudo systemctl enable wg_netmanager".to_string());
        lines.push("".to_string());
        println!("{}", lines.join("\n"));
        Ok(())
    }
    fn arch_specific_init(tx: mpsc::Sender<Event>) {
        simple_signal::set_handler(&[Signal::Int, Signal::Term, Signal::Hup], move |_signals| {
            tx.send(Event::CtrlC).unwrap();
        });
    }
}
