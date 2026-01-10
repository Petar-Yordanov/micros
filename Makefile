LIMINE_DIR ?= $(PWD)/Limine
DISK      := disk.img
DISK_SIZE := 1G
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

$(DISK):
	@echo "[mk] creating $(DISK) ($(DISK_SIZE))"
	@qemu-img create -f raw $(DISK) $(DISK_SIZE) >/dev/null

.PHONY: format-disk
format-disk: $(DISK)
	@set -e; \
	  echo "[mk] formatting $(DISK) as partitionless FAT16"; \
	  mkfs.fat -F 16 -n MICROS64 $(DISK) >/dev/null; \
	  echo -n "[mk] verify boot sig @510..511: "; \
	  dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1; \
	  echo -n "[mk] BPB bytes/sector (offset 11..12): "; \
	  dd if=$(DISK) bs=1 skip=11 count=2 2>/dev/null | od -An -tx1

.PHONY: populate-disk
populate-disk: $(DISK)
	@set -e; \
	  echo "[mk] populating /hello.txt and /testdir/ (FAT16)"; \
	  tmp=$$(mktemp); \
	  printf "MicrOS says hi!\n" > $$tmp; \
	  mmd   -i $(DISK) ::/testdir || true; \
	  mcopy -i $(DISK) $$tmp ::/hello.txt; \
	  rm -f $$tmp; \
	  sync; \
	  echo "[mk] populate done"

.PHONY: ensure-disk
ensure-disk: $(DISK)
	@set -e; \
	  sig=$$(dd if=$(DISK) bs=1 skip=510 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n'); \
	  if [ "$$sig" = "55aa" ] || [ "$$sig" = "55aa" ]; then \
	    echo "[ok] FAT/VBR boot signature present (0x55AA)"; \
	  else \
	    $(MAKE) format-disk; \
	  fi; \
	  $(MAKE) populate-disk

.PHONY: run
run: $(ISO) ensure-disk
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
