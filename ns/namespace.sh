#!/bin/sh
echo === add namespaces
sudo ip netns add alice
sudo ip netns add bob
echo === done

sudo ip netns list

echo === create veth
sudo ip link add veth0 type veth peer name veth1

echo === list veth
sudo ip link | grep veth

echo === assign to namespace
sudo ip link set veth0 netns alice
sudo ip link set veth1 netns bob

echo === list veth
sudo ip link | grep veth

echo === list veth as alice
sudo ip netns exec alice sudo ip link list | grep veth

echo === list veth as bob
sudo ip netns exec bob sudo ip link list | grep veth

echo === add address to veth for alice
sudo ip netns exec alice sudo ip addr add 10.128.1.1/24 dev veth0
sudo ip netns exec alice sudo ip link set dev veth0 up

echo === add address to veth for bob
sudo ip netns exec bob sudo ip addr add 10.128.1.2/24 dev veth1
sudo ip netns exec bob sudo ip link set dev veth1 up

echo === list veth as alice
sudo ip netns exec alice sudo ip link list | grep veth
sudo ip netns exec alice ifconfig

echo === list veth as bob
sudo ip netns exec bob sudo ip link list | grep veth
sudo ip netns exec bob ifconfig

echo === ping bob from alice
sudo ip netns exec alice ping -c 1 10.128.1.2

echo === run wg_netmanager
tmux split-pane -h sudo ip netns exec alice ../target/debug/wg_netmanager -vvv -c test.yaml wg0 10.1.1.1 alice
tmux split-pane -h sudo ip netns exec bob ../target/debug/wg_netmanager -vvv -c test.yaml wg0 10.1.1.3 bob
sleep 5
sudo ip netns exec bob ping 10.1.1.1

echo === show ifconfig
sudo ip netns exec alice ifconfig
sudo ip netns exec bob ifconfig

echo === del namespaces
sudo ip netns del alice
sudo ip netns del bob
echo === done

sudo ip netns list
