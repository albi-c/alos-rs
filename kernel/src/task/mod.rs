use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::mem::offset_of;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;
use crate::{core_local, cpu, println};
use crate::core_local::core_info;
use crate::lock::{Lock, RwLockGuardMut};
use crate::memory::{address, MemoryFlags, MemorySpace};

const KERNEL_STACK_SIZE: usize = 1 << 16;

const CALLEE_SAVED_REGS: usize = 6;

global_asm!(include_str!("../asm/task.asm"));
unsafe extern "C" {
    #[allow(improper_ctypes)]
    fn _task_switch(task: &mut Task, current: &mut Task);
    #[allow(improper_ctypes)]
    fn _task_switch_continue(task: &mut Task) -> !;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _task_lock_force_unlock() {
    unsafe { TASKS.force_write_unlock() };
}

#[derive(Debug, Default)]
struct Tasks {
    tasks: Vec<Option<Box<Task>>>,

    run_queue: VecDeque<usize>,
    sleep_queue: BTreeMap<usize, usize>,
}

impl Tasks {
    fn get(&self, id: usize) -> Option<&Task> {
        Some(self.tasks.get(id)?.as_ref()?)
    }
    fn get_mut(&mut self, id: usize) -> Option<&mut Task> {
        Some(self.tasks.get_mut(id)?.as_mut()?)
    }

    fn get_disjoint_mut<const N: usize>(&mut self, ids: [usize; N]) -> Option<[&mut Option<Box<Task>>; N]> {
        self.tasks.get_disjoint_mut(ids).ok()
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

impl Index<usize> for Tasks {
    type Output = Task;
    fn index(&self, index: usize) -> &Self::Output {
        self.tasks[index].as_ref().unwrap()
    }
}
impl IndexMut<usize> for Tasks {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.tasks[index].as_mut().unwrap()
    }
}

static TASKS: Lock<Tasks> = Lock::new(Tasks {
    tasks: vec![],

    run_queue: VecDeque::new(),
    sleep_queue: BTreeMap::new(),
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

core_local!(#no_mangle CURRENT_TASK: usize = 0);

fn prepare_task_switch(task: &mut Task) {
    cpu::disable_interrupts();
    // set memory space once implemented
    // set kernel stack in tss
    // set kernel stack in core header
    task.core = core_info().id;
}

pub fn task_switch(next: usize) -> Result<(), usize> {
    // will be unlocked in _task_switch()
    let mut lock = TASKS.write();
    let current_id = CURRENT_TASK.read();
    if next == current_id {
        return Ok(());
    }
    let [current, next] = lock.get_disjoint_mut(
        [current_id, next]).expect("invalid task id");
    let current = current.as_mut().unwrap().as_mut();
    let next = next.as_mut().unwrap().as_mut();
    prepare_task_switch(next);

    unsafe { _task_switch(next, current); }

    // will be unlocked in _task_switch()
    core::mem::forget(lock);
    Ok(())
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

    let stack = MemorySpace::with(|mem| {
        let pages = address::page_count_up(size);
        let phys_addr = mem.phys_alloc(pages).expect("out of physical memory");
        let virt_addr = mem.virt_alloc(pages);
        mem.map_flag_func(phys_addr, virt_addr, pages, |i| if i == 0 {
            MemoryFlags::DEFAULT_RO
        } else {
            MemoryFlags::DEFAULT_RW
        });
        unsafe { core::slice::from_raw_parts_mut(virt_addr as *mut usize, size / size_of::<usize>()) }
    });

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
        assert!(offset_of!(Task, id) == 40);
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

    pub fn current() -> usize {
        CURRENT_TASK.read()
    }
}
