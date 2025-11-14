//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
use crate::task::TaskStatus;
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
    pub fn fetch(&mut self, current_pid: Option<usize>) -> Option<Arc<TaskControlBlock>> {
        // self.ready_queue.pop_front()
        //println!("fetch called");
         //self.ready_queue.pop_front()
        if self.ready_queue.is_empty() {
            println!("queue empty");
            return None;
        }

        //let current_pid = current_task().unwrap().pid.0; // 避免 inner_exclusive_access
        //println!("[fetch] current pid = {}", current_pid);
        let mut min_stride = usize::MAX;
        let mut min_index = 0;
        let mut index = 0;
        // find min stride and switch
        for task in self.ready_queue.iter() {
            if Some(task.pid.0) == current_pid {
                index += 1;
                continue;
            }
            let stride = {
                let inner = task.inner_exclusive_access();
                let s = inner.stride;
                drop(inner); 
                s
            };

            if stride < min_stride {
                min_stride = stride;
                min_index = index;
            }
            index += 1;
        }

        let next_task = {
            let next = self.ready_queue.remove(min_index).unwrap();
            next
        };

        // update status and stride
        //let current_task = current_task().unwrap();
        // 获取当前任务
        /*{
            let mut inner = current_task.inner_exclusive_access();
            if inner.task_status == TaskStatus::Running {
                inner.task_status = TaskStatus::Ready;
            }
            drop(inner);
        }



        if !self.ready_queue.iter().any(|t| t.pid.0 == current_pid) {
            self.ready_queue.push_back(current_task.clone());
        }*/
        {
            let mut next_inner = next_task.inner_exclusive_access();
            next_inner.task_status = TaskStatus::Running;
            next_inner.stride += next_inner.pass;
            drop(next_inner);
        }

        Some(next_task)
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
pub fn fetch_task(current_pid: Option<usize>) -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
     TASK_MANAGER.exclusive_access().fetch(current_pid)
}
