//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.stride_schedule()
		//self.ready_queue.pop_front()
    }
	fn stride_schedule(&mut self)-> Option<Arc<TaskControlBlock>>
	{
		let bigstride:usize=65535;
		let index=self.ready_queue.iter().enumerate().min_by_key(
									|(_, tck)| tck.inner_exclusive_access().stride).map(|(i, _)| i)?;
		let task = self.ready_queue.remove(index)?;
		{
            // 获取内部可变引用（作用域结束时自动释放）
            let mut inner = task.inner_exclusive_access();
            
            // 更新 stride（处理溢出）
            inner.stride = inner.stride.wrapping_add(bigstride/inner.priority);
        }
        Some(task)
	}
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
