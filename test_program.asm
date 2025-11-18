[bits 64]

_start:
    mov rax, 0
    mov rdi, 1
    mov rsi, 2
    mov rdx, 3
    mov r8, 4
    mov r9, 5
    mov r10, 6
    o64 syscall

	mov rax, 1
	mov rdi, 1
	lea rsi, [rel message]
	mov rdx, message_end - message
	o64 syscall

	mov rax, 2
	mov rdi, 0
	o64 syscall

	jmp $

message:
	db "Hello, World!", 10
message_end:
