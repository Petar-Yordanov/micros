ENTRY(_start);

SECTIONS
{
  /* Userland: keep it low */
  . = 0x0000000000400000;

  .text : ALIGN(16) { *(.text .text.*) }
  .rodata : ALIGN(16) { *(.rodata .rodata.*) }
  .data : ALIGN(16) { *(.data .data.*) }
  .bss : ALIGN(16) { *(.bss .bss.* COMMON) }

  /DISCARD/ : { *(.eh_frame .eh_frame_hdr) }
}
