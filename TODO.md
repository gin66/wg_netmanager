Todos:
* udp: validate sender to be a valid one
* eliminate the need to specify two listen ports (one could be sufficient with appropriate scheme)
* exchange known client list
* routing
* send default gateway
* monitor connection to peers and remove them, if no connection anymore
* exchange info about new public listeners and update config-file
* add command line option to generate a new config 
* warn about readbility of config file if not -rw-------
* check availability of used shell commands (ip/wg)
* include boringtun, if kernel does not provide wireguard interface
* find a solution for sudo
* share unbound UDP socket addresses between clients
* put udp comm port info into advertisement
