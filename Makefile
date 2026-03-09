LIMINE_DIR ?= $(PWD)/Limine

DISK      := disk.img
DISK_SIZE := 1G

FS ?=

ISO       := micros64.iso
KERNEL    := target/x86_64-unknown-none/debug/micros64
DISK_ABS  := $(abspath $(DISK))

TARGET_JSON := $(abspath x86_64-unknown-none.json)

# ---- Userland build configuration ----
USER_TARGET_DIR := $(abspath src/user/target)

USER_INIT_DIR    := src/user/init
USER_WM_DIR      := src/user/wm
USER_IPC_SRV_DIR := src/user/apps/ipc_srv
USER_IPC_CLI_DIR := src/user/apps/ipc_cli

USER_INIT_BIN    := $(USER_TARGET_DIR)/x86_64-unknown-none/debug/init
USER_WM_BIN      := $(USER_TARGET_DIR)/x86_64-unknown-none/debug/wm
USER_IPC_SRV_BIN := $(USER_TARGET_DIR)/x86_64-unknown-none/debug/ipc_srv
USER_IPC_CLI_BIN := $(USER_TARGET_DIR)/x86_64-unknown-none/debug/ipc_cli

.PHONY: all iso
all: iso
iso: $(ISO)

.PHONY: build-kernel
build-kernel: $(KERNEL)

$(KERNEL):
	@echo "[mk] building kernel -> $(KERNEL)"
	cargo +nightly build -Z build-std=core,alloc

.PHONY: build-user
build-user: $(USER_INIT_BIN) $(USER_WM_BIN) $(USER_IPC_SRV_BIN) $(USER_IPC_CLI_BIN)

# Build init
$(USER_INIT_BIN):
	@echo "[mk] building user init -> $(USER_INIT_BIN)"
	@mkdir -p $(USER_TARGET_DIR)
	@cd $(USER_INIT_DIR) && \
	  cargo +nightly build \
	    -Z build-std=core,alloc \
	    -Z json-target-spec \
	    --target $(TARGET_JSON) \
	    --target-dir $(USER_TARGET_DIR)
	@test -f "$(USER_INIT_BIN)" || (echo "[mk][ERR] expected init bin missing: $(USER_INIT_BIN)"; exit 1)

# Build wm
$(USER_WM_BIN):
	@echo "[mk] building user wm -> $(USER_WM_BIN)"
	@mkdir -p $(USER_TARGET_DIR)
	@cd $(USER_WM_DIR) && \
	  cargo +nightly build \
	    -Z build-std=core,alloc \
	    -Z json-target-spec \
	    --target $(TARGET_JSON) \
	    --target-dir $(USER_TARGET_DIR)
	@test -f "$(USER_WM_BIN)" || (echo "[mk][ERR] expected wm bin missing: $(USER_WM_BIN)"; exit 1)

# Build ipc_srv
$(USER_IPC_SRV_BIN):
	@echo "[mk] building user ipc_srv -> $(USER_IPC_SRV_BIN)"
	@mkdir -p $(USER_TARGET_DIR)
	@cd $(USER_IPC_SRV_DIR) && \
	  cargo +nightly build \
	    -Z build-std=core,alloc \
	    -Z json-target-spec \
	    --target $(TARGET_JSON) \
	    --target-dir $(USER_TARGET_DIR)
	@test -f "$(USER_IPC_SRV_BIN)" || (echo "[mk][ERR] expected ipc_srv bin missing: $(USER_IPC_SRV_BIN)"; exit 1)

# Build ipc_cli
$(USER_IPC_CLI_BIN):
	@echo "[mk] building user ipc_cli -> $(USER_IPC_CLI_BIN)"
	@mkdir -p $(USER_TARGET_DIR)
	@cd $(USER_IPC_CLI_DIR) && \
	  cargo +nightly build \
	    -Z build-std=core,alloc \
	    -Z json-target-spec \
	    --target $(TARGET_JSON) \
	    --target-dir $(USER_TARGET_DIR)
	@test -f "$(USER_IPC_CLI_BIN)" || (echo "[mk][ERR] expected ipc_cli bin missing: $(USER_IPC_CLI_BIN)"; exit 1)

$(ISO): $(KERNEL) limine.conf limine.cfg
	@echo "[mk] building ISO -> $(ISO)"
	xorriso -as mkisofs \
	  -V "MICROS64" -R -J \
	  -o $(ISO) \
	  -b limine-bios-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table \
	  --efi-boot limine-uefi-cd.bin \
	  -efi-boot-part --efi-boot-image --protective-msdos-label \
	  -graft-points \
	  /limine.conf=limine.conf \
	  /limine.cfg=limine.cfg \
	  /boot/limine.conf=limine.conf \
	  /boot/limine.cfg=limine.cfg \
	  /boot/kernel.elf=$(KERNEL) \
	  /boot/limine-bios.sys=$(LIMINE_DIR)/limine-bios.sys \
	  /limine-bios-cd.bin=$(LIMINE_DIR)/limine-bios-cd.bin \
	  /limine-uefi-cd.bin=$(LIMINE_DIR)/limine-uefi-cd.bin
	$(LIMINE_DIR)/limine bios-install $(ISO)

.PHONY: check-vars check-fs
check-vars:
	@set -e; \
	  if [ -z "$(DISK)" ] || [ -z "$(DISK_SIZE)" ]; then \
	    echo "[mk][ERR] DISK/DISK_SIZE not set (DISK='$(DISK)' DISK_SIZE='$(DISK_SIZE)')"; \
	    exit 1; \
	  fi

check-fs:
	@set -e; \
	  if [ -z "$(FS)" ]; then \
	    echo "[mk][ERR] FS is required: make <target> FS=fat32|ext2"; \
	    exit 1; \
	  fi; \
	  if [ "$(FS)" != "fat32" ] && [ "$(FS)" != "ext2" ]; then \
	    echo "[mk][ERR] FS must be fat32 or ext2 (got: $(FS))"; \
	    exit 1; \
	  fi

$(DISK):
	@test -f "$(DISK)" || qemu-img create -f raw $(DISK) $(DISK_SIZE) >/dev/null

.PHONY: format-disk populate-disk

format-disk: check-vars check-fs $(DISK)
	@set -e; \
	  if [ "$(FS)" = "fat32" ]; then \
	    echo "[mk] formatting $(DISK) as partitionless FAT32"; \
	    mkfs.fat -F 32 -n MICROS64 $(DISK) >/dev/null; \
	    echo -n "[mk] verify boot sig @510..511: "; \
	    dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1; \
	  elif [ "$(FS)" = "ext2" ]; then \
	    echo "[mk] formatting $(DISK) as partitionless ext2"; \
	    mkfs.ext2 -F -b 4096 -L MICROS64 $(DISK) >/dev/null; \
	    echo -n "[mk] verify ext2 magic @1080..1081: "; \
	    dd if=$(DISK) bs=1 skip=1080 count=2 2>/dev/null | od -An -tx1; \
	  fi

populate-disk: check-vars check-fs $(DISK) build-user
	@test -f "$(USER_INIT_BIN)" || (echo "[mk][ERR] missing user bin: $(USER_INIT_BIN)"; exit 1)
	@test -f "$(USER_WM_BIN)" || (echo "[mk][ERR] missing user bin: $(USER_WM_BIN)"; exit 1)
	@test -f "$(USER_IPC_SRV_BIN)" || (echo "[mk][ERR] missing user bin: $(USER_IPC_SRV_BIN)"; exit 1)
	@test -f "$(USER_IPC_CLI_BIN)" || (echo "[mk][ERR] missing user bin: $(USER_IPC_CLI_BIN)"; exit 1)
	@set -e; \
	  if [ "$(FS)" = "fat32" ]; then \
	    echo "[mk] populating disk (FAT32): /hello.txt, /testdir/, /bin/init.elf, /bin/wm.elf, /bin/ipc_srv.elf, /bin/ipc_cli.elf"; \
	    tmp=$$(mktemp); \
	    printf "MicrOS says hi!\n" > $$tmp; \
	    mmd   -D o -i $(DISK) ::/testdir >/dev/null 2>&1 || true; \
	    mcopy -D o -o -i $(DISK) $$tmp               ::/hello.txt       >/dev/null 2>&1 || true; \
	    mmd   -D o -i $(DISK) ::/bin                 >/dev/null 2>&1 || true; \
	    mcopy -D o -o -i $(DISK) $(USER_INIT_BIN)    ::/bin/init.elf    >/dev/null 2>&1 || true; \
	    mcopy -D o -o -i $(DISK) $(USER_WM_BIN)      ::/bin/wm.elf      >/dev/null 2>&1 || true; \
	    mcopy -D o -o -i $(DISK) $(USER_IPC_SRV_BIN) ::/bin/ipc_srv.elf >/dev/null 2>&1 || true; \
	    mcopy -D o -o -i $(DISK) $(USER_IPC_CLI_BIN) ::/bin/ipc_cli.elf >/dev/null 2>&1 || true; \
	    rm -f $$tmp; \
	    sync; \
	    echo "[mk] populate done"; \
	  elif [ "$(FS)" = "ext2" ]; then \
	    echo "[mk] populating disk (ext2 via debugfs): /hello.txt, /testdir/, /bin/init.elf, /bin/wm.elf, /bin/ipc_srv.elf, /bin/ipc_cli.elf"; \
	    tmp=$$(mktemp); \
	    printf "MicrOS says hi!\n" > $$tmp; \
	    debugfs -w -R "mkdir /testdir" $(DISK) >/dev/null 2>&1 || true; \
	    debugfs -w -R "write $$tmp /hello.txt" $(DISK) >/dev/null; \
	    debugfs -w -R "mkdir /bin" $(DISK) >/dev/null 2>&1 || true; \
	    debugfs -w -R "write $(USER_INIT_BIN) /bin/init.elf" $(DISK) >/dev/null; \
	    debugfs -w -R "write $(USER_WM_BIN) /bin/wm.elf" $(DISK) >/dev/null; \
	    debugfs -w -R "write $(USER_IPC_SRV_BIN) /bin/ipc_srv.elf" $(DISK) >/dev/null; \
	    debugfs -w -R "write $(USER_IPC_CLI_BIN) /bin/ipc_cli.elf" $(DISK) >/dev/null; \
	    rm -f $$tmp; \
	    sync; \
	    echo "[mk] populate done"; \
	  fi

.PHONY: create-disk setup-disk recreate-disk

create-disk: check-vars check-fs
	@set -e; \
	  echo "[mk] creating fresh disk ($(DISK_SIZE)) FS=$(FS)"; \
	  rm -f $(DISK); \
	  qemu-img create -f raw $(DISK) $(DISK_SIZE) >/dev/null; \
	  echo "[mk] create-disk done"

setup-disk: check-vars check-fs $(DISK)
	@set -e; \
	  echo "[mk] setting up disk FS=$(FS)"; \
	  $(MAKE) format-disk FS=$(FS) DISK=$(DISK) DISK_SIZE=$(DISK_SIZE); \
	  $(MAKE) populate-disk FS=$(FS) DISK=$(DISK) DISK_SIZE=$(DISK_SIZE); \
	  echo "[mk] setup-disk done (FS=$(FS))"

recreate-disk: check-vars check-fs
	@$(MAKE) create-disk FS=$(FS) DISK=$(DISK) DISK_SIZE=$(DISK_SIZE)
	@$(MAKE) setup-disk  FS=$(FS) DISK=$(DISK) DISK_SIZE=$(DISK_SIZE)

.PHONY: infer-fs
infer-fs: check-vars
	@set -e; \
	  if [ ! -f "$(DISK)" ]; then echo ""; exit 0; fi; \
	  sig=$$(dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	  if [ "$$sig" = "55aa" ]; then echo "fat32"; exit 0; fi; \
	  magic=$$(dd if=$(DISK) bs=1 skip=1080 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	  if [ "$$magic" = "53ef" ]; then echo "ext2"; exit 0; fi; \
	  echo ""; exit 0

.PHONY: run run-just
run: run-just

run-just: $(ISO) check-vars
	@set -e; \
	  fs="$$( $(MAKE) -s infer-fs DISK=$(DISK) DISK_SIZE=$(DISK_SIZE) )"; \
	  if [ -z "$$fs" ]; then \
	    echo "[mk][ERR] $(DISK) is missing or unformatted."; \
	    echo "           Do:"; \
	    echo "             make create-disk FS=fat32|ext2"; \
	    echo "             make setup-disk  FS=fat32|ext2"; \
	    exit 1; \
	  fi; \
	  echo "[mk] inferred FS=$$fs for $(DISK)"; \
	  echo "[mk] run-just: not modifying disk"; \
	  qemu-system-x86_64 \
	        -M q35 -m 512M -display gtk \
	        -serial stdio -monitor none \
	        -boot order=d,menu=on \
	        -cdrom $(ISO) \
	        -d int,guest_errors,cpu_reset -D qemu.log -device isa-debug-exit \
	        -blockdev driver=file,filename=$(DISK_ABS),node-name=fi0 \
	        -blockdev driver=raw,file=fi0,node-name=vd0 \
	        -device virtio-blk-pci,drive=vd0,disable-legacy=on \
	        -device virtio-keyboard-pci,disable-legacy=on \
	        -device virtio-mouse-pci,disable-legacy=on \
	        -no-reboot -no-shutdown

.PHONY: clean clean-kernel clean-user clean-iso clean-disk clean-qemu-log
clean: clean-kernel clean-user clean-iso clean-disk clean-qemu-log

clean-kernel:
	@echo "[mk] cleaning kernel (cargo clean at repo root)"
	cargo clean

clean-user:
	@echo "[mk] cleaning userland target dir -> $(USER_TARGET_DIR)"
	@rm -rf $(USER_TARGET_DIR)

clean-iso:
	@echo "[mk] removing ISO -> $(ISO)"
	@rm -f $(ISO)

clean-disk:
	@echo "[mk] removing disk -> $(DISK)"
	@rm -f $(DISK)

clean-qemu-log:
	@rm -f qemu.log
