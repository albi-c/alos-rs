.extern syscall_entry

.global _syscall_entry

// number in rdi
// arguments in rsi, rdx, r10, r8, r9
_syscall_entry:
	swapgs
	mov gs:[24], rsp
	mov rsp, gs:[16]

	pushfq
	push rcx
	push r10
	push r11
	push r12
	mov rcx, r10

	mov r12, gs:[24]
	sti
	cld

	call syscall_entry

	cli
	mov gs:[24], r12

	pop r12
	pop r11
	pop r10
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
