# ARMv9 App provisioning DEMO for islet

## Requirments

Install `virt-manager` to setup networking for QEMU. It can be done manually by calling some iptables, ip and brctl magic but using `virsh` is just way easier.

    sudo apt install virt-manager

Setting up NAT networking for QEMU
    
    sudo virsh net-define config/nat100.xml
    sudo virsh net-start nat100
    sudo virsh net-autostart nat100

Creating TAP device
    
    sudo tunctl -t tap100 -u `whoami`
    sudo ip link set tap100 up
    sudo brctl addif virbr100 tap100

## Building

Download all thirdparty code and the ARM toolchain

    make deps

Compile everything
    
    make compile-image


## Running the DEMO

### Setup image registry

    git clone https://github.com/Havner/image-registry.git
    cd image-registry/server

#### Create an exmaple application image

Pull a container for exmaple nginx

    sudo docker pull --platform linux/arm64 nginx
    mkdir registry
    sudo docker image save -o registry/203ad06a-5098-4d92-ac38-0108eade3b52.tar nginx

Calculate the hash of the docker manifest, here it acts as the root of trust of the image

    tar -axf registry/203ad06a-5098-4d92-ac38-0108eade3b52.tar manifest.json -O | sha256sum


Create our manifest (registry/203ad06a-5098-4d92-ac38-0108eade3b52.json) with content:

    {"uuid":"203ad06a-5098-4d92-ac38-0108eade3b52","name":"nginx","vendor":"nginx","media_type":"Docker","root_of_trust":"__ROOT_OF_TRUST__"}

Replace `__ROOT_OF_TRUST__` with the calculated value and start the registry server by 

    cargo run

### Configure and run a realm

#### Start the host daemon

in `vm/` run:

    QEMU_BIN=../tools/qemu/build/qemu-system-aarch64 RUST_LOG=debug cargo run -- -c socket

#### Connect to the daemon and run commands

    socat - UNIX-CONNECT:socket

Define a realm 

    vm create-realm -i r0 -k ../linux/arch/arm64/boot/Image -v 10

Define an application and install using the registry (by uuid)

    vm create-application -i a0 -r r0 -p 203ad06a-5098-4d92-ac38-0108eade3b52

Check the configuration 

    vm list-realms

Launch the configure realm

    vm launch-realm -i r0

#### Check the log from realm's console

     tail -f workdir/r0/console.log

#### Finishing

If everything worked you shloud see this in realm console

```
[2024-04-19T08:54:01Z INFO  app_manager] Mounting overlays
[2024-04-19T08:54:01Z INFO  app_manager::manager] Mounting overlay for a0
[2024-04-19T08:54:01Z DEBUG app_manager::app] Mounting overlay lower="/workdir/a0/main", upper="/workdir/a0/secure/data", work="/workdir/a0/secure/work", target="/workdir/a0/root"
[    7.677984] overlayfs: upper fs does not support RENAME_WHITEOUT.
[    7.678608] overlayfs: failed to set xattr on upper
[    7.678817] overlayfs: ...falling back to redirect_dir=nofollow.
[    7.679134] overlayfs: ...falling back to uuid=null.
[2024-04-19T08:54:01Z INFO  app_manager] Launcing applications
[2024-04-19T08:54:01Z INFO  app_manager::manager] Launching: a0
[2024-04-19T08:54:01Z INFO  app_manager] Starting event loop
[2024-04-19T08:54:01Z INFO  handler::docker::launcher] stdout: /docker-entrypoint.sh: /docker-entrypoint.d/ is not empty, will attempt to perform configuration

[2024-04-19T08:54:01Z INFO  handler::docker::launcher] stdout: /docker-entrypoint.sh: /docker-entrypoint.d/ is not empty, will attempt to perform configuration
    /docker-entrypoint.sh: Looking for shell scripts in /docker-entrypoint.d/

[2024-04-19T08:54:01Z INFO  handler::docker::launcher] stdout: /docker-entrypoint.sh: /docker-entrypoint.d/ is not empty, will attempt to perform configuration
    /docker-entrypoint.sh: Looking for shell scripts in /docker-entrypoint.d/
    /docker-entrypoint.sh: Launching /docker-entrypoint.d/10-listen-on-ipv6-by-default.sh

[2024-04-19T08:54:02Z INFO  handler::docker::launcher] stdout: /docker-entrypoint.sh: /docker-entrypoint.d/ is not empty, will attempt to perform configuration
    /docker-entrypoint.sh: Looking for shell scripts in /docker-entrypoint.d/
    /docker-entrypoint.sh: Launching /docker-entrypoint.d/10-listen-on-ipv6-by-default.sh
    10-listen-on-ipv6-by-default.sh: info: ipv6 not available
```


It means that the nginx container has started successfully and is working properly. To check that it is indeed responding to http request you can connect to it using the realm ip.
