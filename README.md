Wireguard network manager
=========================

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

peers:
  - publicIp: 192.168.1.70
    wgPort: 50000
    adminPort: 55555
    wgIp: 10.1.1.1
  - publicIp: 192.168.1.212
    wgPort: 50000
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

# Status

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
- [ ] Connection established between two dynamic nodes using their visible outside connections

Routing
- [X] Sets up routes to reach all network participants
- [ ] Gateway feature of nodes

System integration
- [ ] systemd
- [X] rc-based system

Admin
- [X] TUI interface
- [ ] REST API
- [ ] Web UI frontend


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
