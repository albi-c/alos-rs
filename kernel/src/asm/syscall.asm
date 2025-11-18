// [extern "C" fn(...) -> u64]
.extern SYSCALL_TABLE
// usize
.extern SYSCALL_TABLE_LENGTH

.global _syscall_entry

// number in rax
// arguments in rdi, rsi, rdx, r10, r8, r9
_syscall_entry:
	swapgs
	mov gs:[24], rsp
	mov rsp, gs:[16]

	pushfq
	push rcx
	push r11
	push r12

	mov r12, gs:[24]
	sti
	cld

	cmp rax, [SYSCALL_TABLE_LENGTH]
	jae 0f

    mov rax, [SYSCALL_TABLE + 8 * rax]
    test rax, rax
    jz 0f

    mov rcx, r10
	call rax

    jmp 1f
0:
    mov rax, -3
1:
	cli
	mov gs:[24], r12

	pop r12
	pop r11
	xor r10d, r10d
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
