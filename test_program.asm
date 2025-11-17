[bits 64]

_start:
	mov rdi, 1
	mov rsi, 1
	lea rdx, [rel message]
	mov r10, message_end - message
	o64 syscall

	mov rdi, 2
	o64 syscall

	jmp $

message:
	db "Hello, World!", 10
message_end:
