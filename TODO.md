Todos:
* routing
* send default gateway
* exchange info about new public listeners and update config-file
* add command line option to generate a new config 
* warn about readbility of config file if not -rw-------
* check availability of used shell commands (ip/wg)
* include boringtun, if kernel does not provide wireguard interface
* find a solution for sudo
* Retrieve endpoint from clients out of wg parsed output
* rename publicIp to e.g. visibleHost or reachableHost
* more endpoints per peer
* try local endpoints
* add dns
* tui: remove outter frame
* restarting wg_manager on one machine will not cause other machines to replace new public key
* provide a REST interface, so by ssh'ing in any machine it is possibly to retrieve a ascii qrcode and use this as log in for pure wireguard client
* allow the possibility to read config from stdin.
  This would allow to ssh in a machine and start wg_manager without storing the shared key on the filesystem

DONE:
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

NO PRIORITY:
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)

OBSOLETE:
* udp: validate sender to be a valid one
  => through use of encryption, the sender should be legit
* Two listeners cannot communicate as both use the same private ip
  => replaced with symmetrically encrypted udp communication
