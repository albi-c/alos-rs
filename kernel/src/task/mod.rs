use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::collections::btree_map::Entry;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::mem::{offset_of, ManuallyDrop};
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::{core_local, cpu};
use crate::core_local::core_info;
use crate::gdt::tss_set_kernel_stack;
use crate::lock::Lock;
use crate::memory::{address, MemoryFlags, MemorySpace};

const KERNEL_STACK_SIZE: usize = 1 << 16;

const CALLEE_SAVED_REGS: usize = 6;

global_asm!(include_str!("../asm/task.asm"));
unsafe extern "C" {
    #[allow(improper_ctypes)]
    fn _task_switch(task: &mut Task, current: &mut Task);
    #[allow(improper_ctypes)]
    fn _task_switch_continue(task: &mut Task) -> !;

    fn _switch_to_ring_3(func: u64, stack: NonNull<u8>) -> !;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _task_lock_force_unlock() {
    unsafe { TASKS.force_write_unlock() };
}

#[derive(Debug)]
struct ShortVec<T> {
    first: T,
    rest: Option<Vec<T>>,
}

impl<T> ShortVec<T> {
    pub fn new(item: T) -> Self {
        Self {
            first: item,
            rest: None,
        }
    }
    pub fn push(&mut self, item: T) {
        self.rest.get_or_insert_default().push(item);
    }
    pub fn extend_to(self, queue: &mut VecDeque<T>) {
        queue.push_back(self.first);
        if let Some(rest) = self.rest {
            queue.extend(rest);
        }
    }
}

#[derive(Debug, Default)]
struct Tasks {
    tasks: Vec<Option<Box<Task>>>,

    run_queue: VecDeque<usize>,
    sleep_queue: BTreeMap<usize, ShortVec<usize>>,
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

core_local!(#no_mangle CURRENT_TASK: usize = usize::MAX);
core_local!(IDLE_TASK: usize = usize::MAX);
static TIME: AtomicUsize = AtomicUsize::new(0);
// in time ticks (1 ms)
const TASK_RUN_TIME: usize = 10;

pub fn time_tick() {
    let time = TIME.fetch_add(1, Ordering::Relaxed) + 1;
    #[cfg(not(feature = "smp"))] {
        if !CURRENT_TASK.initialized() {
            return;
        }
        let task = CURRENT_TASK.read();
        if task == usize::MAX {
            return;
        }
        if TASKS.read().get(task).unwrap().time <= time {
            sched_yield();
        }
    }
    #[cfg(feature = "smp")] {
        const { panic!("smp not supported"); }
    }
}

pub fn time_now() -> usize {
    TIME.load(Ordering::Relaxed)
}

fn prepare_task_switch(task: &mut Task) {
    cpu::disable_interrupts();
    task.memory_space.clone().make_current();
    tss_set_kernel_stack(task.kernel_stack_start.as_ptr() as u64);
    core_info().syscall_kernel_stack.set(task.kernel_stack_start);
    task.core = core_info().id;
}

pub fn task_switch(next: usize) -> Result<(), usize> {
    // will be unlocked in _task_switch()
    let mut lock = ManuallyDrop::new(TASKS.write());
    let current_id = CURRENT_TASK.read();
    if next == current_id {
        return Ok(());
    }
    let [current, next] = lock.get_disjoint_mut(
        [current_id, next]).expect("invalid task id");
    let current = current.as_mut().unwrap().as_mut();
    let next = next.as_mut().unwrap().as_mut();
    prepare_task_switch(next);
    next.time = time_now() + TASK_RUN_TIME;
    next.state = TaskState::Running;

    unsafe { _task_switch(next, current); }

    // will be unlocked in _task_switch()
    Ok(())
}

pub fn init<T: Sized>(func: extern "C" fn(Box<T>) -> !, param: Box<T>) -> ! {
    let task = Task::new_kernel(Some((func, param)), "init".to_owned());
    let id = task.add_to_tasks(false);

    // will be unlocked in _task_switch_continue()
    let mut lock = ManuallyDrop::new(TASKS.write());
    let task = lock.get_mut(id).expect("invalid task id for init task");
    prepare_task_switch(task);
    task.time = time_now() + TASK_RUN_TIME;
    task.state = TaskState::Running;
    CURRENT_TASK.write(id);
    unsafe { _task_switch_continue(task) }
}

fn sched_next(to_run_queue: bool) {
    let mut lock = TASKS.write();
    let time = time_now();
    while let Some(entry) = lock.sleep_queue.first_entry() {
        if *entry.key() > time {
            break;
        }
        let (_, tasks) = entry.remove_entry();
        tasks.extend_to(&mut lock.run_queue);
    }
    if let Some(next) = lock.run_queue.pop_front() {
        if to_run_queue {
            lock.run_queue.push_back(CURRENT_TASK.read());
        }
        drop(lock);
        task_switch(next).expect("invalid next task");
    } else if to_run_queue {
        drop(lock);
        return;
    } else {
        drop(lock);
        task_switch(IDLE_TASK.read()).expect("invalid idle task");
    }
}

pub fn sched_yield() {
    sched_next(true);
}

pub fn sched_sleep(time: usize) {
    if time == 0 {
        return;
    }
    let mut lock = TASKS.write();
    match lock.sleep_queue.entry(time_now() + time) {
        Entry::Vacant(entry) => {
            entry.insert(ShortVec::new(CURRENT_TASK.read()));
        },
        Entry::Occupied(mut entry) => {
            entry.get_mut().push(CURRENT_TASK.read());
        },
    }
    Task::with_current_mut(|task| {
        task.state = TaskState::Blocked;
    });
    sched_next(false);
}

pub fn sched_exit() {
    let id = CURRENT_TASK.read();
    let mut lock = TASKS.write();
    lock.get_mut(id).unwrap().state = TaskState::Terminated;
    // TODO: remove from TASKS.tasks
    sched_next(false);
}

extern "C" fn kernel_stack_underflow() -> ! {
    panic!("kernel stack underflow - task function returned")
}

#[derive(Debug)]
#[repr(C)]
pub struct Task {
    pub kernel_stack: NonNull<u8>,
    pub kernel_stack_base: NonNull<u8>,
    pub kernel_stack_start: NonNull<u8>,

    pub user_stack: *mut u8,
    pub user_stack_base: *mut u8,

    pub time: usize,
    pub core: usize,

    pub id: usize,
    pub state: TaskState,
    pub flags: u32,

    pub memory_space: Arc<MemorySpace>,
    pub name: String,
    pub next: Option<&'static Task>,
    pub files: Vec<FileDescriptor>,
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
    let offset = stack.len() - (INIT_VALUES + CALLEE_SAVED_REGS);
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

fn allocate_user_stack(mem: &MemorySpace, size: usize) -> (NonNull<u8>, NonNull<u8>) {
    let size = address::page_align_up(size);
    let base = {
        let pages = address::page_count_up(size);
        let phys_addr = mem.user_phys_alloc(pages).expect("out of physical memory");
        let virt_addr = mem.user_virt_alloc(pages).expect("no user virtual memory space");
        mem.map_flag_func(phys_addr, virt_addr, pages, |i| if i == 0 {
            MemoryFlags::DEFAULT_RO | MemoryFlags::USER
        } else {
            MemoryFlags::DEFAULT_RW | MemoryFlags::USER
        });
        NonNull::new(virt_addr as *mut u8).unwrap()
    };

    let top = unsafe { base.byte_add(size) };
    (top, base)
}

pub fn switch_to_ring_3(func: u64) -> ! {
    let stack = Task::with_current(|task| NonNull::new(task.user_stack).unwrap());
    unsafe { _switch_to_ring_3(func, stack) }
}

// TODO: drop implementation for stacks
impl Task {
    pub const FLAG_KERNEL: u32 = 1 << 0;
    pub const FLAG_IDLE: u32 = 1 << 1;

    const fn check_struct_offsets() {
        assert!(offset_of!(Task, kernel_stack) == 0);
        assert!(offset_of!(Task, id) == 56);
    }

    pub fn new_kernel<T: Sized>(func: Option<(extern "C" fn(Box<T>) -> !, Box<T>)>, name: String) -> Box<Self> {
        const { Self::check_struct_offsets() };

        let is_idle = func.is_none();
        let (kernel_stack, kernel_stack_base) = allocate_kernel_stack(
            KERNEL_STACK_SIZE, func);
        Box::new(Self {
            kernel_stack,
            kernel_stack_base,
            kernel_stack_start: kernel_stack,

            user_stack: core::ptr::null_mut(),
            user_stack_base: core::ptr::null_mut(),

            time: 0,
            core: 0,

            id: 0,
            state: TaskState::Ready,
            flags: Self::FLAG_KERNEL | if is_idle { Self::FLAG_IDLE } else { 0 },

            memory_space: MemorySpace::get().new(None),
            name,
            next: None,
            files: vec![],
        })
    }

    pub fn new_user<T: Sized>(func: (extern "C" fn(Box<T>) -> !, Box<T>), name: String,
                              memory_space: Arc<MemorySpace>, stack_size: usize) -> Box<Self> {
        const { Self::check_struct_offsets() };

        let (kernel_stack, kernel_stack_base) = allocate_kernel_stack(
            KERNEL_STACK_SIZE, Some(func));
        let (user_stack, user_stack_base) = allocate_user_stack(
            &memory_space, stack_size);
        Box::new(Self {
            kernel_stack,
            kernel_stack_base,
            kernel_stack_start: kernel_stack,

            user_stack: user_stack.as_ptr(),
            user_stack_base: user_stack_base.as_ptr(),

            time: 0,
            core: 0,

            id: 0,
            state: TaskState::Ready,
            flags: 0,

            memory_space,
            name,
            next: None,
            files: vec![],
        })
    }

    pub fn add_to_tasks(self: Box<Self>, add_to_run_queue: bool) -> usize {
        let mut lock = TASKS.write();
        let id = lock.insert(self);
        if add_to_run_queue {
            lock.run_queue.push_back(id);
        }
        id
    }

    pub fn current() -> usize {
        CURRENT_TASK.read()
    }

    pub fn with_current<T>(func: impl FnOnce(&Task) -> T) -> T {
        let lock = TASKS.read();
        let task = lock.get(CURRENT_TASK.read()).unwrap();
        func(task)
    }
    pub fn with_current_mut<T>(func: impl FnOnce(&mut Task) -> T) -> T {
        let mut lock = TASKS.write();
        let task = lock.get_mut(CURRENT_TASK.read()).unwrap();
        func(task)
    }
}
