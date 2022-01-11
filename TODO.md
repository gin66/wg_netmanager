Todos:
* routing
* send default gateway
* add command line option to generate a new config 
* warn about readbility of config file if not -rw-------
* check availability of used shell commands (ip/wg)
* include boringtun, if kernel does not provide wireguard interface
* find a solution for sudo
* rename publicIp to e.g. visibleHost or reachableHost
* share info about visible endpoint/admin udp port
	+ Retrieve endpoint from clients out of wg showconf parsed output
* add dns
* make video with watch wg showconf per vps
* Support https://github.com/FlyveHest/wg-friendly-peer-names
* Add mobile client type: is connection initiator. not reachable from outside
* Validate commandline ip against subnet
* Add option to set a static route to loopback to prevent leakage of packets to default gateway 
* Route delete does not work as expected. in one case temp. direct link was down, but the route for it wasn't
* wg_netmanager has started after a while, sending advertisement to public endpoint
	=> route was missing

DONE:
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
* exchange info about new public listeners and update config-file
* allow the possibility to read config from stdin.
  This would allow to ssh in a machine and start wg_manager without storing the shared key on the filesystem
* provide a REST interface, so by ssh'ing in any machine it is possibly to retrieve a ascii qrcode and use this as log in for pure wireguard client
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)
* more endpoints per peer

OBSOLETE:
* udp: validate sender to be a valid one
  => through use of encryption, the sender should be legit
* Two listeners cannot communicate as both use the same private ip
  => replaced with symmetrically encrypted udp communication
