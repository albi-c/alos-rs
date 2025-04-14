    cli
    cld

    push rsi
    push rcx

    mov rsi, [rsp + 16]
    mov rcx, [rsp + 8]
    mov [rsp + 16], rcx
    mov rcx, [rsp]
    mov [rsp + 8], rcx
    add rsp, 8

    mov rcx, [rsp + 16]

    push r8
    xor r8, r8

    push r9
    mov r9, [rsp + 48]

    ?push_all

    mov rdi, ?i
    mov rdx, cr2
    sti
    call exc_handler
    cli

    ?pop_all

    pop r9
    pop r8
    pop rcx
    pop rsi

    iretq
