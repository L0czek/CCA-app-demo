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

Compile everything and run QEMU
    
    make run

To run QEMU and drop shell inside initramfs

    make run EXEC=shell

To run without recompiling 
    
    make run-only

