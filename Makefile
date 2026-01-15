LIMINE_DIR ?= $(PWD)/Limine

DISK      := disk.img
DISK_SIZE := 1G

# FS is mandatory for disk actions:
#   make create-disk FS=fat32
#   make create-disk FS=ext2
FS ?=

ISO       := micros64.iso
KERNEL    := target/x86_64-unknown-none/debug/micros64
DISK_ABS  := $(abspath $(DISK))

all: $(ISO)

$(KERNEL):
	cargo +nightly build -Z build-std=core,alloc

$(ISO): $(KERNEL) limine.conf limine.cfg
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

.PHONY: check-fs
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
	@echo "[mk] creating $(DISK) ($(DISK_SIZE))"
	@qemu-img create -f raw $(DISK) $(DISK_SIZE) >/dev/null

.PHONY: create-disk
create-disk: check-fs
	@set -e; \
	  echo "[mk] recreating disk ($(DISK_SIZE)) FS=$(FS)"; \
	  rm -f $(DISK); \
	  qemu-img create -f raw $(DISK) $(DISK_SIZE) >/dev/null; \
	  $(MAKE) format-disk FS=$(FS); \
	  $(MAKE) populate-disk FS=$(FS); \
	  echo "[mk] create-disk done (FS=$(FS))"

.PHONY: format-disk
format-disk: check-fs $(DISK)
	@set -e; \
	  if [ "$(FS)" = "fat32" ]; then \
	    echo "[mk] formatting $(DISK) as partitionless FAT32"; \
	    mkfs.fat -F 32 -n MICROS64 $(DISK) >/dev/null; \
	    echo -n "[mk] verify boot sig @510..511: "; \
	    dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1; \
	    echo -n "[mk] BPB bytes/sector (offset 11..12): "; \
	    dd if=$(DISK) bs=1 skip=11 count=2 2>/dev/null | od -An -tx1; \
	    echo -n "[mk] BPB root_clus (FAT32, offset 44..47): "; \
	    dd if=$(DISK) bs=1 skip=44 count=4 2>/dev/null | od -An -tx1; \
	  elif [ "$(FS)" = "ext2" ]; then \
	    echo "[mk] formatting $(DISK) as partitionless ext2"; \
	    mkfs.ext2 -F -b 4096 -L MICROS64 $(DISK) >/dev/null; \
	    echo -n "[mk] verify ext2 magic @1080..1081: "; \
	    dd if=$(DISK) bs=1 skip=1080 count=2 2>/dev/null | od -An -tx1; \
	  fi

.PHONY: populate-disk
populate-disk: check-fs $(DISK)
	@set -e; \
	  if [ "$(FS)" = "fat32" ]; then \
	    echo "[mk] populating /hello.txt and /testdir/ (FAT32)"; \
	    tmp=$$(mktemp); \
	    printf "MicrOS says hi!\n" > $$tmp; \
        mmd   -D o -i $(DISK) ::/testdir >/dev/null 2>&1 || true; \
        mcopy -D o -o -i $(DISK) $$tmp ::/hello.txt >/dev/null 2>&1 || true; \
	    rm -f $$tmp; \
	    sync; \
	    echo "[mk] populate done"; \
	  elif [ "$(FS)" = "ext2" ]; then \
	    echo "[mk] populating /hello.txt and /testdir/ (ext2 via debugfs)"; \
	    tmp=$$(mktemp); \
	    printf "MicrOS says hi!\n" > $$tmp; \
	    debugfs -w -R "mkdir /testdir" $(DISK) >/dev/null 2>&1 || true; \
	    debugfs -w -R "write $$tmp /hello.txt" $(DISK) >/dev/null; \
	    rm -f $$tmp; \
	    sync; \
	    echo "[mk] populate done"; \
	  fi

.PHONY: ensure-disk
ensure-disk: check-fs $(DISK)
	@set -e; \
	  if [ "$(FS)" = "fat32" ]; then \
	    sig=$$(dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	    if [ "$$sig" = "55aa" ]; then \
	      echo "[ok] FAT VBR boot signature present (0x55AA)"; \
	    else \
	      echo "[mk] disk not FAT (boot sig missing) -> recreating"; \
	      $(MAKE) create-disk FS=$(FS); \
	      exit 0; \
	    fi; \
	    $(MAKE) populate-disk FS=$(FS); \
	  elif [ "$(FS)" = "ext2" ]; then \
	    magic=$$(dd if=$(DISK) bs=1 skip=1080 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	    if [ "$$magic" = "53ef" ]; then \
	      echo "[ok] ext2 superblock magic present (0xEF53)"; \
	    else \
	      echo "[mk] disk not ext2 (magic missing) -> recreating"; \
	      $(MAKE) create-disk FS=$(FS); \
	      exit 0; \
	    fi; \
	    $(MAKE) populate-disk FS=$(FS); \
	  fi

.PHONY: infer-fs
infer-fs: $(DISK)
	@set -e; \
	  sig=$$(dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	  if [ "$$sig" = "55aa" ]; then \
	    echo "fat32"; exit 0; \
	  fi; \
	  magic=$$(dd if=$(DISK) bs=1 skip=1080 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	  if [ "$$magic" = "53ef" ]; then \
	    echo "ext2"; exit 0; \
	  fi; \
	  echo ""; exit 0

.PHONY: run
run: $(ISO) $(DISK)
	@set -e; \
	  fs="$$( $(MAKE) -s infer-fs )"; \
	  if [ -z "$$fs" ]; then \
	    echo "[mk][ERR] cannot infer FS from $(DISK). Create one explicitly:"; \
	    echo "           make create-disk FS=fat32   OR   make create-disk FS=ext2"; \
	    exit 1; \
	  fi; \
	  echo "[mk] inferred FS=$$fs for $(DISK)"; \
	  $(MAKE) ensure-disk FS=$$fs; \
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
		-no-reboot -no-shutdown \
		-device virtio-mouse-pci,disable-legacy=on

.PHONY: clean
clean:
	cargo clean
	rm -f $(ISO) $(DISK)
