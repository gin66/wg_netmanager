MID PRIORITY:
* send default gateway
* add command line option to generate a new config 
* warn about readbility of config file if not -r--------
* check availability of used shell commands (ip/wg)
* find a solution for sudo
* add dns
* avoid syncconf with identical configuration (can happen if several nodes are added at once)
* Support https://github.com/FlyveHest/wg-friendly-peer-names
* Add mobile client type: is connection initiator. not reachable from outside
* Add legacy mode for traditional wireguard clients
* Reduce ip-load (how?) for keep alive packages
* Measure the connection speed to select the gateway based on this citeria
* use netlink_sys/netlink_packet-wireguard/netlink-packet-route to remove need for ip/wg shell calls

DONE:
* Replace lots of unwrap() with friendly error code
* Refactor code and combine node/dynamic peer into one structure. Simply NetManager and run loop
* rename bring_up() to create_device()
* routing
* share info about visible endpoint/admin udp port
	+ Retrieve endpoint from clients out of wg showconf parsed output
* Validate commandline ip against subnet
* Route delete does not work as expected. in one case temp. direct link was down, but the route for it wasn't
* wg_netmanager has started after a while, sending advertisement to public endpoint
	=> route was missing
* send update advertisement to all direct peers on route change
	=> faster distribution of info
* try local endpoints
* restarting wg_manager on one machine will not cause other machines to replace new public key
* tui: remove outter frame
* lastseen is a weird timestamp. looks like uptime
* add option to use an existing wireguard interface
* put udp comm port/socketaddr info into advertisement
* refactor UdpPacket enum
* exchange known peer list
* Public Key should have a timestamp
* simplify RouteInfo for Peers
* Do not send advertisement to self
* add github actions
* monitor connection to peers and remove them, if no connection anymore
* udp package encryption
* README
* add crc check to udp send/receive
* Add time to udp packet and check time window to mitigate replay attack

LOW PRIORITY:
* include boringtun, if kernel does not provide wireguard interface
	or https://git.zx2c4.com/wireguard-rs (more official)
* How to detect a net.ipv6.bindv6only=1 system
* Try to use a SIP-router and remove the need for a static server
* exchange info about new public listeners and update config-file
  This would allow to ssh in a machine and start wg_manager without storing the shared key on the filesystem
* provide a REST interface, so by ssh'ing in any machine it is possibly to retrieve a ascii qrcode and use this as log in for pure wireguard client
* more endpoints per peer
* make video with watch wg showconf per vps
* send out shutdown info to all nodes

OBSOLETE:
* Add option to set a static route to loopback to prevent leakage of packets to default gateway 
  => Add default route to wireguard interface
* allow the possibility to read config from stdin.
  => use /dev/stdin as filename under linux
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)
  => Need two
* rename publicIp to e.g. visibleHost or reachableHost
  => replaced by endpoint
* udp: validate sender to be a valid one
  => through use of encryption, the sender should be legit
* Two listeners cannot communicate as both use the same private ip
  => replaced with symmetrically encrypted udp communication
