Todos:
* README
* udp: validate sender to be a valid one
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
* Two listeners cannot communicate as both use the same private ip
* Retrieve endpoint from clients out of wg parsed output
* rename publicIp to e.g. visibleHost
* Add time to udp packet and check time window to mitigate replay attack

DONE:
* monitor connection to peers and remove them, if no connection anymore

NO PRIORITY:
* udp package encryption
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)
