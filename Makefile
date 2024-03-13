
ROOT_DIR             = $(shell git rev-parse --show-toplevel)
CONFIG_DIR           = $(ROOT_DIR)/config
DOWNLOAD_DIR         = $(ROOT_DIR)/.download
TOOLCHAIN_DIR        = $(ROOT_DIR)/toolchain
TOOLS_DIR            = $(ROOT_DIR)/tools

GNU_TOOLCHAIN_URL    = https://developer.arm.com/-/media/Files/downloads/gnu/13.2.rel1/binrel/arm-gnu-toolchain-13.2.rel1-x86_64-aarch64-none-linux-gnu.tar.xz
GNU_TOOLCHAIN_DIR    = $(TOOLCHAIN_DIR)/aarch64-none-linux-gnu/bin

KERNEL_UPSTREAM    = https://github.com/torvalds/linux
KERNEL_DIR         = $(ROOT_DIR)/linux

BUSYBOX_UPSTREAM   = https://github.com/mirror/busybox.git
BUSYBOX_DIR        = $(TOOLS_DIR)/busybox

STRACE_UPSTREAM    = https://github.com/strace/strace.git
STRACE_DIR         = $(TOOLS_DIR)/strace

GDB_UPSTREAM       = https://ftp.gnu.org/gnu/gdb/gdb-14.1.tar.xz
GDB_DIR            = $(TOOLS_DIR)/gdb

APP_MANAGER_DIR    = $(ROOT_DIR)/app-manager
APP_MANAGER_BIN    = $(APP_MANAGER_DIR)/target/aarch64-unknown-linux-gnu/debug/app-manager

INITRAMFS_DIR      = $(ROOT_DIR)/initramfs

QEMU_TAP_DEVICE    ?= tap100
QEMU_VSOCK_CID     ?= 100
EXEC               ?= app-manager

GREEN_COLOR = \\033[0;32m
RED_COLOR   = \\033[0;31m
NC          = \\033[0;m

export PATH := $(GNU_TOOLCHAIN_DIR):$(PATH)

makedir:
	@for d in "$(DOWNLOAD_DIR)" "$(TOOLCHAIN_DIR)"; do \
		[ -d "$$d" ] || mkdir -p "$$d"; \
	done


toolchains: makedir
	@echo "$(GREEN_COLOR)Fetching gnu toolchain.$(NC)"
	@[ -f "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz" ] || \
		wget "$(GNU_TOOLCHAIN_URL)" -O "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz"
	@echo "$(GREEN_COLOR)Decompressing.$(NC)"
	@tar xf "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz" -C "$(TOOLCHAIN_DIR)"
	@rm -rf "$(TOOLCHAIN_DIR)/aarch64-none-linux-gnu"
	@mv toolchain/*aarch64-none-linux-gnu* toolchain/aarch64-none-linux-gnu

fetch-linux-kernel:
	@echo "$(GREEN_COLOR)Fetching Linux kernel source.$(NC)"
	@[ -d "$(KERNEL_DIR)" ] || git clone --depth=1 $(KERNEL_UPSTREAM) $(KERNEL_DIR)

fetch-busybox:
	@echo "$(GREEN_COLOR)Fetching busybox source.$(NC)"
	@[ -d "$(BUSYBOX_DIR)" ] || git clone --depth=1 $(BUSYBOX_UPSTREAM) $(BUSYBOX_DIR)

fetch-strace:
	@echo "$(GREEN_COLOR)Fetching strace source.$(NC)"
	@[ -d "$(STRACE_DIR)" ] || git clone --depth=1 $(STRACE_UPSTREAM) $(STRACE_DIR)

fetch-gdb:
	@echo "$(GREEN_COLOR)Fetching gdb source.$(NC)"
	@[ -f "$(DOWNLOAD_DIR)/gdb.tar.xz" ] || \
		wget "$(GDB_UPSTREAM)" -O "$(DOWNLOAD_DIR)/gdb.tar.xz"
	@echo "$(GREEN_COLOR)Decompressing.$(NC)"
	@tar xf "$(DOWNLOAD_DIR)/gdb.tar.xz" -C "$(TOOLS_DIR)"
	@rm -rf "$(GDB_DIR)"
	@mv $(TOOLS_DIR)/*gdb* "$(TOOLS_DIR)/gdb"

deps: toolchains fetch-linux-kernel fetch-busybox fetch-strace fetch-gdb

compile-busybox: $(BUSYBOX_DIR)/busybox $(CONFIG_DIR)/busybox.config
	@echo "$(GREEN_COLOR)Building busybox.$(NC)"
	@[ -f "$(BUSYBOX_DIR)/.config" ] || cp -v $(CONFIG_DIR)/busybox.config $(BUSYBOX_DIR)/.config
	@ARCH=aarch64 CROSS_COMPILE=aarch64-none-linux-gnu- \
		$(MAKE) -C $(BUSYBOX_DIR) -j $(shell nproc)

compile-strace: $(STRACE_DIR)
	@echo "$(GREEN_COLOR)Building strace.$(NC)"
	@if [ ! -f "$(STRACE_DIR)/Makefile" ]; then \
		cd $(STRACE_DIR) && \
			./bootstrap && \
			./configure --build x86_64-pc-linux-gnu --host aarch64-none-linux-gnu \
				LDFLAGS="-static -pthread" --enable-mpers=check; \
	fi;
	@$(MAKE) -C "$(STRACE_DIR)" -j $(shell proc)

compile-gdbserver: $(GDB_DIR)
	@echo "$(GREEN_COLOR)Building gdbserver.$(NC)"
	@if [ ! -f "$(GDB_DIR)/build/Makefile" ]; then \
		mkdir "$(GDB_DIR)/build"; \
		PATH=$(GNU_TOOLCHAIN_DIR):$$PATH \
			cd "$(GDB_DIR)/build" && \
			$(GDB_DIR)/configure \
					--host="aarch64-none-linux-gnu" \
					--enable-gdbserver \
					--disable-gdb \
					--disable-docs \
					--disable-binutils \
					--disable-gas \
					--disable-sim \
					--disable-gprof \
					--disable-inprocess-agent \
					--prefix="$(GDB_DIR)/bin" \
					CC="aarch64-none-linux-gnu-gcc" \
					CXX="aarch64-none-linux-gnu-g++" \
					LDFLAGS="-static -static-libstdc++"; \
	fi;
	@$(MAKE) -C "$(GDB_DIR)/build" -j $(shell nproc)

compile-app-manager: $(APP_MANAGER_DIR)
	@echo "$(GREEN_COLOR)Building app-manager.$(NC)"
	@cd $(APP_MANAGER_DIR) && cargo build --target=aarch64-unknown-linux-gnu

prepare-initramfs: compile-busybox compile-strace compile-gdbserver compile-app-manager
	@echo "$(GREEN_COLOR)Preparing initramfs.$(NC)"
	@[ -d "$(INITRAMFS_DIR)" ] || mkdir "$(INITRAMFS_DIR)"
	@mkdir -p "$(INITRAMFS_DIR)/bin"
	@cp -v "$(BUSYBOX_DIR)/busybox" "$(INITRAMFS_DIR)/bin"
	@cp -v "$(STRACE_DIR)/src/strace" "$(INITRAMFS_DIR)/bin"
	@cp -v "$(GDB_DIR)/build/gdbserver/gdbserver" "$(INITRAMFS_DIR)/bin"
	@cp -v "$(APP_MANAGER_BIN)" "$(INITRAMFS_DIR)/bin"
	@cp -v "$(CONFIG_DIR)/udhcpc.script" "$(INITRAMFS_DIR)/bin"
	@cp -v "$(CONFIG_DIR)/init" "$(INITRAMFS_DIR)"

compile-image: prepare-initramfs $(KERNEL_DIR)
	@echo "$(GREEN_COLOR)Building kernel image.$(NC)"
	@[ -f "$(BUSYBOX_DIR)/.config" ] || cp -v "$(CONFIG_DIR)/kernel.config" "$(KERNEL_DIR)/.config"
	@ARCH=arm64 CROSS_COMPILE=aarch64-none-linux-gnu- $(MAKE) -C "$(KERNEL_DIR)" Image -j $(shell nproc)

run: compile-image
	@echo "$(GREEN_COLOR)Running QEMU.$(NC)"
	qemu-system-aarch64 \
		-machine virt \
		-cpu cortex-a57 \
		-nographic -smp 1 \
		-kernel $(KERNEL_DIR)/arch/arm64/boot/Image \
		-append "console=ttyAMA0" \
		-m 2048 \
		-netdev tap,id=mynet0,ifname=$(QEMU_TAP_DEVICE),script=no,downscript=no \
		-device e1000,netdev=mynet0,mac=52:55:00:d1:55:01 \
		-append $(EXEC)

run-only: 
	@echo "$(GREEN_COLOR)Running QEMU.$(NC)"
	qemu-system-aarch64 \
		-machine virt \
		-cpu cortex-a57 \
		-nographic -smp 1 \
		-kernel $(KERNEL_DIR)/arch/arm64/boot/Image \
		-append "console=ttyAMA0" \
		-m 2048 \
		-netdev tap,id=mynet0,ifname=$(QEMU_TAP_DEVICE),script=no,downscript=no \
		-device e1000,netdev=mynet0,mac=52:55:00:d1:55:01 \
		-device vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid=$(QEMU_VSOCK_CID) \
		-append $(EXEC)

	
