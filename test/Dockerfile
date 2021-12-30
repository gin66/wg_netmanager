FROM alpine:3.14.3

RUN apk update; \
	apk add wireguard-tools curl git gcc; \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs >install.sh; \
	chmod a+x install.sh; \
	./install.sh -y;

ENV ENV="/root/.cargo/env"

RUN cd; \
    source .cargo/env; \
	git clone https://github.com/gin66/wg_netmanager.git; \
	cd wg_netmanager; \
	cargo build

EXPOSE 50000/udp
