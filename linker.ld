/* Higher-half static kernel for Limine 10
   VMA = 0xffffffff80000000 (higher half)
   Segments: RX / R / RW, 4K aligned
*/
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

PHDRS {
  text   PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data   PT_LOAD FLAGS(6);
}

SECTIONS
{
  /* place the image in the higher half */
  . = 0xffffffff80000000;

  .text : ALIGN(4K) {
    *(.text .text.*)
  } :text

  .rodata : ALIGN(4K) {
    *(.rodata .rodata.*)
  } :rodata

  .data : ALIGN(4K) {
    *(.data .data.*)
  } :data

  .bss : ALIGN(4K) {
    *(.bss .bss.* COMMON)
  } :data

  /DISCARD/ : {
    *(.eh_frame*)
    *(.comment*)
    *(.note*)
  }
}
