// CoreLocal<usize>
.extern CURRENT_TASK
// fn()
.extern _task_lock_force_unlock

// fn(&Task, &Task)
.global _task_switch
// fn(&Task) -> !
.global _task_switch_continue

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

	mov [rsi + 0], rsp

	mov r12, [CURRENT_TASK]
	mov rsi, [rdi + 40]
	mov gs:[r12], rsi

_task_switch_continue:
	mov rsp, [rdi + 0]

	call _task_lock_force_unlock

	pop r15
	pop r14
	pop r13
	pop r12
	pop rbp
	pop rbx

	pop rdi

	ret
