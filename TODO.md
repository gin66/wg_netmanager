Todos:
* exchange known client list
* routing
* send default gateway
* exchange info about new public listeners and update config-file
* add command line option to generate a new config 
* warn about readbility of config file if not -rw-------
* check availability of used shell commands (ip/wg)
* include boringtun, if kernel does not provide wireguard interface
* find a solution for sudo
* share unbound UDP socket addresses between clients
* put udp comm port info into advertisement
* Retrieve endpoint from clients out of wg parsed output
* rename publicIp to e.g. visibleHost
* Add time to udp packet and check time window to mitigate replay attack
* Do not sent advertisement to self

DONE:
* monitor connection to peers and remove them, if no connection anymore
* udp package encryption
* README

NO PRIORITY:
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)

OBSOLETE:
* udp: validate sender to be a valid one
  => through use off encryption
* Two listeners cannot communicate as both use the same private ip
  => replaced with encrypted udp communication
