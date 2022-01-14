Wireguard network manager
=========================

# Status: TESTING

# Motivation / Problem

Situation is a couple of vps, some IoT devices in the home network and couple of roaming laptops/iPads/smartphones.  Till now a quite complex setup involving wireguard, openvpn, ssh'ing from box to box was in use.

Using wireguard all the time is not an option, because then the Laptop in the home network would route traffic to the box one meter away via the long route (the vps somewhere in the Internet), instead of using the short path within the home network.

An alternative could have been [tinc](https://tinc-vpn.org/), but tinc - licensed under GPL v2 - may never hit iOs/iPadOS AppStore. So not a solution.

# Solution / Idea

Use wireguard and add the missing management part. This shall:
- bring up and configure wireguard devices
- perform the key management/exchange
- set up routes automagically
- identify shorter routes

Instead of manually creating keys (which is still the case in gui versions) the network uses one file to be distributed to all participants.  Here a work in progress example (filename net.yaml):

```yaml
network:
  sharedKey: YDUBM6FhERePZ4gPlxzAbCN7K61BPjy7HApWYL+P128=
  subnet: 10.1.1.0/8

peers:
  - endPoint: 192.168.1.70:50000
    adminPort: 55555
    wgIp: 10.1.1.1
  - endPoint: 192.168.1.212:50000
    adminPort: 55555
    wgIp: 10.1.1.2
```

The sharedKey can be created with `wg genkey`.


Unfortunately there is still the need to define at least ONE static peer.

Bring up e.g. alice, bob and charlie would be just:
```
ALICE'S   BOX> wg_netmanager -c net.yaml wg0 10.1.1.1  alice
BOB'S     BOX> wg_netmanager -c net.yaml wg0 10.1.1.16 bob
CHARLIE'S BOX> wg_netmanager -c net.yaml wg0 10.1.1.21 charlie
```
with alice being reachable on 192.168.1.70

# Definitions/Nomenclatura

- Node: Participant in the network
- Peer: Node reachable directly via wireguard interface
- Dynamic peer: synonym for peer
- Static server: Fixed address/port either in internet or in intranet
- Dynamic server: Any node (24/7) behind a firewall
- Roaming client: Any node, which is switching IP-address
- Mobile client: Roaming client with data volume limitation

# Status

Node support:
- [X] Static server in config file
- [X] Static server per dynamic dns
- [ ] Static server per command line parameter
- [X] Dynamic server
- [X] Roaming client
- [ ] Mobile client

OS-Support
- [X] Linux
- [ ] MacOS
- [ ] Windows
- [ ] iOS/iPadOS
- [ ] Android
- [ ] FreeBSD
- [ ] OpenBSD

Wireguard-Interface
- [X] Kernel-driver + Command ip/wg
- [X] wiregard-go + Command ip/wg
- [X] boringtun + Command ip/wg
- [ ] boringtun (embedded) + ip

Network
- [X] Connection established between two static servers (fixed address)
- [X] Connection established between dynamic node and static server
- [X] Connection established between two dynamic nodes in same subnet
- [X] Connection established between two dynamic nodes using their visible outside connections aka NAT traversal

Routing
- [X] Sets up routes to reach all network participants
- [ ] Gateway feature of nodes

System integration
- [ ] systemd
- [ ] rc-based system

Admin
- [X] TUI interface
- [ ] REST API
- [ ] Web UI frontend

# Installation

With rust installed, just issue
```
	cargo install wg_netmanager
```

# Usage

Generally, a shared key need to be created and stored under sharedKey in your local copy of network.yaml. This can be done by:
```
	wg genkey
```
and copy the result in the network.yaml

Then modify the peers list to accommodate your setup. At least one peer with a static address is needed. For dyndns-reachable servers, use the hostname instead of an ip.

If the subnet 10.1.1.0/8 does not suit your needs, then change it. All wireguard IPs need to be included in the chosen subnet.

Then copy the final yaml file to all your nodes and start the wg_netmanager with:
```
	wg_netmanager -c network.yaml <wireguard-interface> <wireguard-ip> <name>
```


For vps, which do not support wireguard as network interface, either boringtun or wireguard-go can be used. Then inform wg_netmanager about the pre-configured wireguard interface with the `-e` commandline switch.

For a list of commandline options, just use `--help` as usual.

# Testing

Using namespaces several boxes can be simulated on one linux machine.
See as [example](https://github.com/gin66/wg_netmanager/blob/main/ns/three_boxes.sh)

# Technical Background

wg_manager will add and delete routes on demand on two levels:
- As routing policy of the kernel using `ip route add <ip>/32 dev <wg_dev>`
- If a node is directly reachable, by adding a peer entry in the wireguard configuration
  with a list of allowed ip's. This list includes the peer and all further nodes, for which this peer can forward traffic to.

# Security Consideration

In case one node of this wireguard network is compromised, then the implications are severe. The symmetric key can be distributed and any attacker's node can join the network.

With the current implementation an attacker could even issue this command:
```
ATTACKER'S BOX> wg_netmanager -c net.yaml wg0 8.8.8.8  alice
```
And ALL network participants will start to route any DNS request addressed to 8.8.8.8 to the ATTACKER's box.

This is actually a very cool feature and on the other hand quite frightening.


## Update

Depending on the linux version, `ip route add` wants to find an interface with the corresponding subnet. If not successful, then it will throw a "no such process error".

Remedy is, that the wireguard interface is associated with IP and resp. netmask. without adding a route:
```
  ip addr add 10.1.1.1/24 dev wg0 noprefixroute
```

Consequently, in the config-file the subnet has to specified. If the subnet does not include 8.8.8.8, then other nodes will not accept it - unless the defined subnet includes 8.8.8.8
