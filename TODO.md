Todos:
* exchange known peer list
* routing
* send default gateway
* exchange info about new public listeners and update config-file
* add command line option to generate a new config 
* warn about readbility of config file if not -rw-------
* check availability of used shell commands (ip/wg)
* include boringtun, if kernel does not provide wireguard interface
* find a solution for sudo
* share unbound UDP socket addresses between clients
* put udp comm port/socketaddr info into advertisement
* Retrieve endpoint from clients out of wg parsed output
* rename publicIp to e.g. visibleHost or reachableHost
* Do not sent advertisement to self
* Public Key should have a timestamp
* more endpoints per peer
* simplify RouteInfo for Peers

DONE:
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
