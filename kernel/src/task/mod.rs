use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::mem::offset_of;
use core::ptr::NonNull;
use crate::{core_local, cpu};
use crate::core_local::core_info;
use crate::lock::Lock;

const KERNEL_STACK_SIZE: usize = 1 << 16;

const CALLEE_SAVED_REGS: usize = 6;

global_asm!(include_str!("../asm/task.asm"));
unsafe extern "C" {
    fn _task_switch(task: &mut Task);
    fn _task_switch_continue(task: &mut Task) -> !;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _task_lock_force_unlock() {
    unsafe { TASKS.force_write_unlock() };
}

#[derive(Debug, Default)]
struct Tasks {
    tasks: Vec<Option<Box<Task>>>,
}

impl Tasks {
    fn get(&self, id: usize) -> Option<&Task> {
        Some(self.tasks.get(id)?.as_ref()?)
    }
    fn get_mut(&mut self, id: usize) -> Option<&mut Task> {
        Some(self.tasks.get_mut(id)?.as_mut()?)
    }

    fn insert(&mut self, task: Box<Task>) -> usize {
        let (id, task) = if let Some((id, t)) = self.tasks.iter_mut().enumerate().find(|(_, t)| t.is_none()) {
            let last = t.replace(task);
            assert!(last.is_none());
            (id, t.as_mut().unwrap())
        } else {
            let id = self.tasks.len();
            let task = self.tasks.push_mut(Some(task)).as_mut().unwrap();
            (id, task)
        };
        task.id = id;
        id
    }
}

static TASKS: Lock<Tasks> = Lock::new(Tasks {
    tasks: Vec::new(),
});

#[derive(Debug)]
pub struct FileDescriptor {}

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum TaskState {
    Running,
    Blocked,
    Ready,
    Terminated,
}

core_local!(#no_mangle CURRENT_TASK: Option<&'static mut Task> = None);

fn prepare_task_switch(task: &mut Task) {
    cpu::disable_interrupts();
    // set memory space once implemented
    // set kernel stack in tss
    // set kernel stack in core header
    task.core = core_info().id;
}

pub fn init<T: Sized>(func: extern "C" fn(Box<T>) -> !, param: Box<T>) -> ! {
    let task = Task::new_kernel(Some((func, param)), "init".to_owned());
    let id = task.add_to_tasks();

    // will be unlocked in _task_switch_continue()
    let mut lock = TASKS.write();
    let task = lock.get_mut(id).expect("invalid task id for init task");
    prepare_task_switch(task);
    // set task time
    task.state = TaskState::Running;
    unsafe { _task_switch_continue(task) }
}

extern "C" fn kernel_stack_underflow() -> ! {
    panic!("kernel stack underflow - task function returned")
}

#[derive(Debug)]
#[repr(C)]
pub struct Task {
    kernel_stack: NonNull<u8>,
    kernel_stack_base: NonNull<u8>,
    kernel_stack_start: NonNull<u8>,

    time: usize,
    core: usize,

    id: usize,
    state: TaskState,
    flags: u32,

    name: String,
    next: Option<&'static Task>,
    files: Vec<FileDescriptor>,
}

fn allocate_kernel_stack<T: Sized>(size: usize, func: Option<(extern "C" fn(Box<T>) -> !, Box<T>)>) -> (NonNull<u8>, NonNull<u8>) {
    const { assert!(size_of::<Box<T>>() == size_of::<usize>()) };
    let mut stack = vec![0usize; size / size_of::<usize>()].into_boxed_slice();
    let base = NonNull::new(stack.as_mut_ptr()).unwrap();
    const INIT_VALUES: usize = 4;
    let offset = stack.len() - (INIT_VALUES + CALLEE_SAVED_REGS + 1);
    let top = NonNull::new(&raw mut stack[offset]).unwrap();
    let (func, param) = func.map_or(
        (0, 0), |(func, param)| (func as usize, Box::leak(param) as *mut _ as usize));
    // param, function, stack underflow function, 0
    stack[offset+CALLEE_SAVED_REGS..offset+CALLEE_SAVED_REGS+INIT_VALUES].copy_from_slice(&[
        param,
        func,
        kernel_stack_underflow as usize,
        0,
    ]);
    (top.cast(), base.cast())
}

impl Task {
    pub const FLAG_KERNEL: u32 = 1 << 0;
    pub const FLAG_IDLE: u32 = 1 << 1;

    const fn check_kernel_stack_struct_offset() {
        assert!(offset_of!(Task, kernel_stack) == 0);
    }

    pub fn new_kernel<T: Sized>(func: Option<(extern "C" fn(Box<T>) -> !, Box<T>)>, name: String) -> Box<Self> {
        const { Self::check_kernel_stack_struct_offset() };

        let is_idle = func.is_none();
        let (kernel_stack, kernel_stack_base) = allocate_kernel_stack(
            KERNEL_STACK_SIZE, func);
        Box::new(Self {
            kernel_stack,
            kernel_stack_base,
            kernel_stack_start: kernel_stack,

            time: 0,
            core: 0,

            id: 0,
            state: TaskState::Ready,
            flags: Self::FLAG_KERNEL | if is_idle { Self::FLAG_IDLE } else { 0 },

            name,
            next: None,
            files: vec![],
        })
    }

    pub fn add_to_tasks(self: Box<Self>) -> usize {
        TASKS.write().insert(self)
    }

    pub fn current() -> &'static mut Task {
        CURRENT_TASK.read().expect("no current task")
    }
}
