// CoreLocal<usize>
.extern CURRENT_TASK

// fn()
.extern _task_lock_force_unlock

.global _task_switch, _task_switch_continue

_task_switch:
	cli
	cld

	push rdi

	push rbx
	push rbp
	push r12
	push r13
	push r14
	push r15

	mov r12, [CURRENT_TASK]
	mov rax, gs:[r12]

	mov [rax + 16], rsp

	mov gs:[r12], rdi

_task_switch_continue:
	mov rsp, [rdi + 16]

	call _task_lock_force_unlock

	pop r15
	pop r14
	pop r13
	pop r12
	pop rbp
	pop rbx

	pop rdi

	ret
