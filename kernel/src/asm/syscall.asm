.extern syscall_entry

.global _syscall_entry

// number in rax
// arguments in rdi, rsi, rdx, r8, r9, r10
_syscall_entry:
	swapgs
	mov gs:[24], rsp
	mov rsp, gs:[16]

	pushfq
	push rcx
	push r11
	push r12
	push r10

	mov r12, gs:[24]
	sti
	cld

    mov rcx, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov rdi, rax
	call syscall_entry

	cli
	mov gs:[24], r12

    pop r10
	pop r12
	pop r11
	xor r9d, r9d
	xor r8d, r8d
	xor esi, esi
	xor edi, edi
	xor edx, edx
	pop rcx
	popfq

	mov rsp, gs:[24]
	swapgs
	sysretq
