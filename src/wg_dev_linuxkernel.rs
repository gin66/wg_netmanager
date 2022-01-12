use std::collections::HashMap;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::{Command, Stdio};

use ipnet::Ipv4Net;
use log::*;

use crate::error::*;
use crate::wg_dev::WireguardDevice;

pub struct WireguardDeviceLinux {
    device_name: String,
    ip: Ipv4Addr,
}
impl WireguardDeviceLinux {
    pub fn init<T: Into<String>>(wg_name: T) -> Self {
        WireguardDeviceLinux {
            device_name: wg_name.into(),
            ip: "0.0.0.0".parse().unwrap(),
        }
    }
    fn internal_execute_command(
        &self,
        mut args: Vec<&str>,
        input: Option<&str>,
    ) -> BoxResult<std::process::Output> {
        let mut args_with_sudo = vec!["sudo"];
        args_with_sudo.append(&mut args);

        let stdin_par = if input.is_none() {
            Stdio::null()
        } else {
            Stdio::piped()
        };

        let child = Command::new(args_with_sudo.remove(0))
            .args(args_with_sudo)
            .stdin(stdin_par)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(input) = input {
            write!(child.stdin.as_ref().unwrap(), "{}", input)
                .map_err(|e| format!("write to child in execute_command: {:?}", e))?;
        }

        let output = child.wait_with_output()?;

        if output.status.success() {
            Ok(output)
        } else {
            #[allow(clippy::try_err)]
            Err(format!(
                "process failed wth {}",
                String::from_utf8_lossy(&output.stderr)
            ))?
        }
    }
    fn execute_command(
        &self,
        args: Vec<&str>,
        input: Option<&str>,
    ) -> BoxResult<std::process::Output> {
        trace!(target: "wireguard", "{:?}", args);
        self.internal_execute_command(args, input).map_err(|e| {
            error!(target: "wireguard", "{:?}",e);
            e
        })
    }
    fn update_conf(&self, conf: &str, set_new: bool) -> BoxResult<()> {
        debug!(target: "wireguard", "Update configuration: {}", conf);
        let wg_cmd = if set_new { "setconf" } else { "syncconf" };

        let args = vec!["mktemp", "/tmp/wg_XXXXXXXXXX"];
        let output = self.execute_command(args, None)?;
        let tmpfname = String::from_utf8_lossy(&output.stdout);
        let fname = tmpfname.trim();
        trace!(target: "wireguard", "temp file {}", fname);

        let _ = self.execute_command(vec!["tee", "-a", &*fname], Some(conf))?;
        let _ = self.execute_command(vec!["wg", wg_cmd, &self.device_name, &*fname], None)?;
        let _ = self.execute_command(vec!["rm", &*fname], None)?;
        Ok(())
    }
}

impl WireguardDevice for WireguardDeviceLinux {
    fn check_device(&self) -> BoxResult<bool> {
        debug!("Check for device {}", self.device_name);
        let result = self.execute_command(vec!["ip", "link", "show", &self.device_name], None);
        Ok(result.is_ok())
    }
    fn bring_up_device(&self) -> BoxResult<()> {
        debug!("Bring up device");
        let _ = self.execute_command(
            vec!["ip", "link", "add", &self.device_name, "type", "wireguard"],
            None,
        );
        debug!("Interface {} created", self.device_name);

        let _ = self.execute_command(vec!["ip", "link", "set", &self.device_name, "up"], None);
        debug!("Interface {} up", self.device_name);

        Ok(())
    }
    fn take_down_device(&self) -> BoxResult<()> {
        debug!("Take down device");
        let _ = self.execute_command(vec!["ip", "link", "del", &self.device_name], None);
        debug!("Interface {} destroyed", self.device_name);
        Ok(())
    }
    fn set_ip(&mut self, ip: &Ipv4Addr, subnet: &Ipv4Net) -> BoxResult<()> {
        debug!("Set IP {}", ip);
        self.ip = *ip;
        let ip_extend = format!("{}/{}", ip, subnet.prefix_len());
        let _ = self.execute_command(
            vec![
                "ip",
                "addr",
                "add",
                &ip_extend,
                "dev",
                &self.device_name,
                "noprefixroute",
            ],
            None,
        );

        debug!("Interface {} set ip", self.device_name);
        Ok(())
    }
    fn add_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()> {
        debug!("Set route {}", route);
        if let Some(gateway) = gateway {
            let _ = self.execute_command(
                vec![
                    "ip",
                    "route",
                    "add",
                    route,
                    "via",
                    &gateway.to_string(),
                    "dev",
                    &self.device_name,
                ],
                None,
            );
        } else {
            let _ = self.execute_command(
                vec![
                    "ip",
                    "route",
                    "add",
                    route,
                    "dev",
                    &self.device_name,
                    //    "src",
                    //    &format!("{}", self.ip),
                ],
                None,
            );
        }
        debug!("Interface {} set route", self.device_name);
        Ok(())
    }
    fn replace_route(&self, route: &str, gateway: Option<Ipv4Addr>) -> BoxResult<()> {
        debug!("Set route {}", route);
        if let Some(gateway) = gateway {
            let _ = self.execute_command(
                vec![
                    "ip",
                    "route",
                    "replace",
                    route,
                    "via",
                    &gateway.to_string(),
                    "dev",
                    &self.device_name,
                ],
                None,
            );
        } else {
            let _ = self.execute_command(
                vec![
                    "ip",
                    "route",
                    "replace",
                    route,
                    "dev",
                    &self.device_name,
                    //    "src",
                    //    &format!("{}", self.ip),
                ],
                None,
            );
        }
        debug!("Interface {} set route", self.device_name);
        Ok(())
    }
    fn del_route(&self, route: &str, _gateway: Option<Ipv4Addr>) -> BoxResult<()> {
        debug!("Set route {}", route);
        let _ = self.execute_command( vec!["ip", "route", "del", route, ], None,);
        debug!("Interface {} deleted route", self.device_name);
        Ok(())
    }
    fn flush_all(&self) -> BoxResult<()> {
        for what in ["route", "addr"] {
            debug!("Flush {}", what);
            let _ = self.execute_command(vec!["ip", what, "flush", "dev", &self.device_name], None);
            debug!("{} flushed", what);
        }
        Ok(())
    }
    fn set_conf(&self, conf: &str) -> BoxResult<()> {
        self.update_conf(conf, true)
    }
    fn sync_conf(&self, conf: &str) -> BoxResult<()> {
        self.update_conf(conf, false)
    }
    fn retrieve_conf(&self) -> BoxResult<HashMap<String, SocketAddr>> {
        let mut pubkey_to_endpoint = HashMap::new();
        let result = self.execute_command(vec!["wg", "showconf", &self.device_name], None)?;
        let wg_config = String::from_utf8_lossy(&result.stdout);
        trace!("{}", wg_config);
        let ini = ini::Ini::load_from_str(&wg_config).unwrap();
        for peer_ini in ini.section_all(Some("Peer")) {
            if let Some(endpoint) = peer_ini.get("Endpoint") {
                if let Some(pub_key) = peer_ini.get("PublicKey") {
                    let sock_addr: SocketAddr = endpoint.parse().unwrap();
                    trace!("{} is endpoint of {}", sock_addr, pub_key);
                    pubkey_to_endpoint.insert(pub_key.to_string(), sock_addr);
                }
            }
        }
        Ok(pubkey_to_endpoint)
    }
    fn create_key_pair(&self) -> BoxResult<(String, String)> {
        let result_priv_key = self.execute_command(vec!["wg", "genkey"], None)?;
        let raw_priv_key = String::from_utf8_lossy(&result_priv_key.stdout);
        let priv_key = raw_priv_key.trim();

        let result_pub_key = self.execute_command(vec!["wg", "pubkey"], Some(priv_key))?;
        let raw_pub_key = String::from_utf8_lossy(&result_pub_key.stdout);
        let pub_key = raw_pub_key.trim();

        Ok((priv_key.to_string(), pub_key.to_string()))
    }
}
