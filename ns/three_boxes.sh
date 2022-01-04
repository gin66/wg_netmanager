#!/bin/sh
SEP="====================="
FAIL="\n\n$SEP\n FAIL\n$SEP\n"

echo === add namespaces
sudo ip netns add backbone
sudo ip netns add alice
sudo ip netns add bob
sudo ip netns add charlie
echo === done

sudo ip netns list

echo === create veth
sudo ip link add veth0_alice type veth peer name veth1_alice
sudo ip link add veth0_bob type veth peer name veth1_bob
sudo ip link add veth0_charlie type veth peer name veth1_charlie

echo === list veth
sudo ip link | grep veth

echo === assign to namespace
sudo ip link set veth0_alice netns alice
sudo ip link set veth0_bob netns bob
sudo ip link set veth0_charlie netns charlie
sudo ip link set veth1_alice netns backbone
sudo ip link set veth1_bob netns backbone
sudo ip link set veth1_charlie netns backbone

echo === list veth
sudo ip link | grep veth

echo === list veth in backbone
sudo ip netns exec backbone ip link add vbr0 type bridge
sudo ip netns exec backbone ip link

echo === build bridge
sudo ip netns exec backbone brctl addif vbr0 veth1_alice
sudo ip netns exec backbone brctl addif vbr0 veth1_bob
sudo ip netns exec backbone brctl addif vbr0 veth1_charlie
sudo ip netns exec backbone ip link set veth1_alice up
sudo ip netns exec backbone ip link set veth1_bob up
sudo ip netns exec backbone ip link set veth1_charlie up
sudo ip netns exec backbone ip link set vbr0 up

echo === list veth
sudo ip link | grep veth
echo === done veth

echo === list veth as alice
sudo ip netns exec alice ip link list | grep veth

echo === list veth as bob
sudo ip netns exec bob ip link list | grep veth

echo === list veth as charlie
sudo ip netns exec charlie ip link list | grep veth

echo === add address to veth for alice
sudo ip netns exec alice ip addr add 10.128.1.1/24 dev veth0_alice
sudo ip netns exec alice ip link set dev veth0_alice up

echo === add address to veth for bob
sudo ip netns exec bob ip addr add 10.128.1.2/24 dev veth0_bob
sudo ip netns exec bob ip link set dev veth0_bob up

echo === add address to veth for charlie
sudo ip netns exec charlie ip addr add 10.128.1.3/24 dev veth0_charlie
sudo ip netns exec charlie ip link set dev veth0_charlie up

echo === Check setup: ping bob from alice
sudo ip netns exec alice ping -c 2 10.128.1.2 || echo -e $FAIL
echo === Check setup: ping charlie from alice
sudo ip netns exec alice ping -c 2 10.128.1.3 || echo -e $FAIL

echo === ping bob from charlie
sudo ip netns exec charlie ping -c 3 10.128.1.2

echo === run wg_netmanager

echo Set up three boxes: alice, bob and charlie
echo alice is listener
echo bob is client
echo charlie is client
echo expectation is, that after a while the ping succeeds: bob can reach charlie via the tunnel

#tmux split-pane -h sh -c "sudo ip netns exec alice ../target/debug/wg_netmanager -vvvvv -c test.yaml wg0 10.1.1.1 alice || sleep 10"
tmux split-pane -h sudo ip netns exec alice ../target/debug/wg_netmanager -vvvvv -c test.yaml wg0 10.1.1.1 alice
tmux split-pane -h sudo ip netns exec bob ../target/debug/wg_netmanager -vvvvv -c test.yaml wg0 10.1.1.3 bob
tmux split-pane -h sudo ip netns exec charlie ../target/debug/wg_netmanager -vvvvv -c test.yaml wg0 10.1.1.4 charlie
sleep 120
sudo ip netns exec bob ping -c 2 10.1.1.1 || echo -e $FAIL
sudo ip netns exec charlie ping -c 2 10.1.1.1 || echo -e $FAIL
sudo ip netns exec alice ping -c 2 10.1.1.3 || echo -e $FAIL
sudo ip netns exec alice ping -c 2 10.1.1.4 || echo -e $FAIL
sudo ip netns exec bob ping -c 2 10.1.1.4 || echo -e $FAIL

echo ==== Kill the test subjects
sudo ip netns exec alice killall sudo
sudo ip netns exec bob killall sudo
sudo ip netns exec charlie killall sudo

#echo === show ifconfig
#sudo ip netns exec alice ifconfig
#sudo ip netns exec bob ifconfig
#sudo ip netns exec charlie ifconfig
#sudo ip netns exec backbone ifconfig

echo === del namespaces
sudo ip netns del alice
sudo ip netns del bob
sudo ip netns del charlie
sudo ip netns del backbone
echo === done

sudo ip netns list
