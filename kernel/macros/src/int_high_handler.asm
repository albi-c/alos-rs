    cli
    cld

    push r9
    mov r9, [rsp + 24]

    ?push_all

    mov rdi, ?i
    mov rsi, r9
    sti
    call irq_handler
    cli

    ?pop_all

    pop r9

    iretq
