use alloc::string::String;
use alloc::vec::Vec;
use crate::core_local;

#[derive(Debug)]
pub struct FileDescriptor {}

#[derive(Debug, Copy, Clone)]
pub enum TaskState {
    Running,
    Blocked,
    Ready,
    Terminated,
}

core_local!(CURRENT_TASK: Option<&'static Task> = None);

pub fn get_current() -> &'static Task {
    CURRENT_TASK.read().expect("no current task")
}

#[derive(Debug)]
pub struct Task {
    name: String,
    next: Option<&'static Task>,
    files: Vec<FileDescriptor>,
}
