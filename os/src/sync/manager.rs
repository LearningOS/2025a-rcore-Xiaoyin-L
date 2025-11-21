
use alloc::vec::Vec;
use alloc::vec;

/// Semaphore and Mutex detetct for oneprocess
pub struct ProcessDeadlockDetector {
    /// [thread_id][resource_id]
    pub allocation: Vec<Vec<usize>>,
    /// [thread_id][resource_id]  
    pub need: Vec<Vec<usize>>,
    /// [resource_id]        
    pub available: Vec<usize>,  
    /// thread_count
    pub thread_count: usize,
    /// resource_count
    pub resource_count: usize,
    /// 分类型处理
    pub resource_types: Vec<ResourceType>,
}

pub enum ResourceType {
    Mutex,          
    Semaphore(usize), 
}

impl ProcessDeadlockDetector {
    /// 初始化
    pub fn new() -> Self {
        Self {
            allocation: Vec::new(),
            need: Vec::new(),
            available: Vec::new(),
            thread_count: 0,
            resource_count: 0,
            resource_types: Vec::new(),
        }
    }

    /// 添加互斥锁资源
    pub fn add_mutex(&mut self) -> usize {
        let resource_id = self.resource_count;
        self.available.push(1);  
        self.resource_types.push(ResourceType::Mutex);
        self.extend_threads();
        self.resource_count += 1;
        resource_id
    }
    
    /// 添加信号量资源
    pub fn add_semaphore(&mut self, initial_count: usize) -> usize {
        let resource_id = self.resource_count;
        self.available.push(initial_count);
        self.resource_types.push(ResourceType::Semaphore(initial_count));
        self.extend_threads();
        self.resource_count += 1;
        resource_id
    }

    /// 扩展所有线程
    fn extend_threads(&mut self) {
        for i in 0..self.thread_count {
            self.allocation[i].push(0);
            
            /*if let Some(resource_type) = self.resource_types.last() {
                let initial_need = match resource_type {
                    ResourceType::Mutex => 1,
                    ResourceType::Semaphore(count) => *count,
                };
                self.need[i].push(initial_need);
            }*/
            self.need[i].push(0);
        }
    }

    /// 安全检查算法
    pub fn check_safty_algo(&self) -> bool {
        let mut work = self.available.clone();
        let mut finish = vec![false; self.thread_count];
        
        loop {
            let mut this_process = false;
            for i in 0..self.thread_count {
                if !finish[i] && self.enough_source(i, &work) {
                    for j in 0..self.resource_count {
                        work[j] += self.allocation[i][j];
                    }
                    finish[i] = true;
                    this_process = true;
                    break;
                }
            }

            if !this_process { break; }   
        }

        finish.iter().all(|&f| f)
    }
    /// 判断资源是否足够
    fn enough_source(&self, thread_id: usize, work: &[usize]) -> bool {
        (0..self.resource_count).all(|j| self.need[thread_id][j] <= work[j])
    }

    /// 安全检查
    pub fn check_safty(&mut self, thread_id: usize, resource_id: usize) -> bool {
       println!("[DEBUG] check_safty: thread_id={}, resource_id={}", thread_id, resource_id);
       println!("[DEBUG] available: {:?}", self.available);
        println!("[DEBUG] allocation: {:?}", self.allocation);
        println!("[DEBUG] need: {:?}", self.need);

        if resource_id >= self.resource_count {
            println!(" Failed: resource_id out of bounds");
            return false;
        }

        
        if thread_id >= self.thread_count {
            while self.allocation.len() <= thread_id {
                self.allocation.push(vec![0; self.resource_count]);

                /*let mut initial_need = Vec::new();
                for resource_type in &self.resource_types {
                    let need = match resource_type {
                        ResourceType::Mutex => 1,
                        ResourceType::Semaphore(count) => *count,
                    };
                    initial_need.push(need);
                }
                self.need.push(initial_need);*/
                self.need.push(vec![0; self.resource_count]);
            }
            self.thread_count = thread_id + 1;
        }

         if let ResourceType::Mutex = self.resource_types[resource_id] {
            if self.allocation[thread_id][resource_id] > 0 {
                return false; // 自我死锁
            }
        }
        
        // ✅ 关键修复：如果没有可用资源，直接拒绝
        if self.available[resource_id] == 0 {
            return false;
        }

        if self.need[thread_id][resource_id] == 0 {
            self.need[thread_id][resource_id] = 1;
        }
        
        // 模拟分配
        self.available[resource_id] -= 1;
        self.allocation[thread_id][resource_id] += 1;
        self.need[thread_id][resource_id] -= 1;
        
        // 安全检查
        let is_safe = self.check_safty_algo();
        
        if !is_safe {
            // 回滚分配
            self.available[resource_id] += 1;
            self.allocation[thread_id][resource_id] -= 1;
            self.need[thread_id][resource_id] += 1;
        }
        
        is_safe
    }

    /// 释放资源
    pub fn release(&mut self, thread_id: usize, resource_id: usize) {
        if thread_id < self.thread_count && resource_id < self.resource_count {
            if self.allocation[thread_id][resource_id] > 0 {
                self.allocation[thread_id][resource_id] -= 1;
                self.available[resource_id] += 1;
                self.need[thread_id][resource_id] += 1;
            }
        }
    }
}
