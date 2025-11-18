[bits 64]

section .text
global _start
_start:
    mov eax, 0
    mov edi, 1
    mov esi, 2
    mov edx, 3
    mov r10d, 4
    mov r8d, 5
    mov r9d, 6
    o64 syscall

    cld
    lea rdi, [rel message]
    lea rsi, [rel message_ro]
    mov ecx, message_ro_end - message_ro
    rep movsb
    mov byte [rel message_end - 2], '!'

	mov eax, 1
	mov edi, 1
	lea rsi, [rel message]
	mov edx, message_end - message
	o64 syscall

	mov eax, 2
	mov edi, 0
	o64 syscall

	jmp $

section .rodata
message_ro:
	db "Hello, World?", 10
message_ro_end:

section .bss
message:
	resb (message_ro_end - message_ro)
message_end:
