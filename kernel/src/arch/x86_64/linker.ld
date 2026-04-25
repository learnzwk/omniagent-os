ENTRY(_start)

SECTIONS {
    . = 1M;

    .text BLOCK(4K) : ALIGN(4K) {
        *(.multiboot)
        *(.text .text.*)
    }

    .rodata BLOCK(4K) : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data BLOCK(4K) : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss BLOCK(4K) : ALIGN(4K) {
        *(COMMON)
        *(.bss .bss.*)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
        *(.comment)
    }

    . = ALIGN(16);
    . += 16K;
    _kernel_stack_top = .;
}
