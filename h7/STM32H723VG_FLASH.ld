ENTRY(Reset_Handler)
MEMORY
{
  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 1024K
  RAM (xrw)  : ORIGIN = 0x24000000, LENGTH = 128K
  /* 主栈固定使用 128 KB DTCM 的顶部 8 KB，其余空间不参与当前链接。 */
  STACK (xrw) : ORIGIN = 0x2001E000, LENGTH = 8K
}
_estack = ORIGIN(STACK) + LENGTH(STACK);
SECTIONS
{
  .isr_vector : { . = ALIGN(4); KEEP(*(.isr_vector)) . = ALIGN(4); } > FLASH
  .text : { *(.text*) *(.rodata*) KEEP(*(.init)) KEEP(*(.fini)) . = ALIGN(4); } > FLASH
  .ARM.extab : { *(.ARM.extab*) } > FLASH
  .ARM.exidx : { __exidx_start = .; *(.ARM.exidx*) __exidx_end = .; } > FLASH
  .preinit_array (READONLY) : { PROVIDE_HIDDEN(__preinit_array_start = .); KEEP(*(.preinit_array*)) PROVIDE_HIDDEN(__preinit_array_end = .); } > FLASH
  .init_array (READONLY) : { PROVIDE_HIDDEN(__init_array_start = .); KEEP(*(SORT(.init_array.*))) KEEP(*(.init_array*)) PROVIDE_HIDDEN(__init_array_end = .); } > FLASH
  .fini_array (READONLY) : { PROVIDE_HIDDEN(__fini_array_start = .); KEEP(*(SORT(.fini_array.*))) KEEP(*(.fini_array*)) PROVIDE_HIDDEN(__fini_array_end = .); } > FLASH
  _sidata = LOADADDR(.data);
  .data : { . = ALIGN(4); _sdata = .; *(.data*) . = ALIGN(4); _edata = .; } > RAM AT> FLASH
  .bss (NOLOAD) : { . = ALIGN(4); _sbss = .; *(.bss*) *(COMMON) . = ALIGN(4); _ebss = .; } > RAM
  .stack (NOLOAD) : { . = ALIGN(8); _stack_limit = .; . += 8K; } > STACK
  /DISCARD/ : { *(.note.GNU-stack) }
}
