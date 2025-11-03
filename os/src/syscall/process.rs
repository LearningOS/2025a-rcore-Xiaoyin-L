//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
use crate::mm::{PTEFlags, write_to_user};
use crate::task::current_user_token;
use crate::timer::get_time_us;

pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    // 仿照sys_write
    // 调用page_table的translated_byte_buffer
    let us = get_time_us();
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };

     match write_to_user(current_user_token(), _ts, &time_val) {
        Ok(()) => 0,
        Err(()) => -1,
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
use crate::mm::VirtAddr;
use crate::task::get_syscall_times;
use crate::mm::PageTable;

pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");

    let page_table = PageTable::from_token(current_user_token());

    match trace_request {
        0 => {
            if id >= (1 << 39) {
                return -1;
            }
            let user_va = VirtAddr::from(id);
            let vpn = user_va.floor();
            let pg_offet = id & (PAGE_SIZE - 1);

            if let Some(pte) = page_table.translate(vpn) {
                if pte.is_valid() && pte.readable() {
                    let py_addr = pte.ppn().0 * PAGE_SIZE + pg_offet;
                    let ptr = py_addr as *const u8;
                    unsafe { *ptr as isize }
                }
                else { -1 }
            }
            else { -1 }
        },
        1 => {
            let user_va = VirtAddr::from(id);
            let vpn = user_va.floor();
            let pg_offet = id & (PAGE_SIZE - 1);

            if let Some(pte) = page_table.translate(vpn) {
                if pte.is_valid() && pte.writable() {
                    let py_addr = pte.ppn().0 * PAGE_SIZE + pg_offet;
                    let ptr = py_addr as *mut u8;
                    unsafe { *ptr = data as u8; }
                    0
                }
                else { -1 }
            } 
            else { -1 }
        },
        2 => {
            get_syscall_times(id) as isize
        },
        _ => {
            -1
        },
    }
}

// YOUR JOB: Implement mmap.
use crate::mm::frame_alloc;
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

    if start % PAGE_SIZE !=0 {
        return -1;
    }
    if port & !0x7 != 0 {
        return -1;
    }
    if port & 0x7 == 0 {
        return -1;
    }

    let token = current_user_token();
    let mut page_table = PageTable::from_token(token);
    // get page count and end addr
    

    // 设置flag
    let mut flags = PTEFlags::V | PTEFlags::U;
    if port & 0x1 != 0 {
        flags |= PTEFlags::R;
    }
    if port & 0x2 != 0 {
        flags |= PTEFlags::W;
    }
    if port & 0x4 != 0 {
        flags |= PTEFlags::X;
    }
    if len == 0 {
        return 0;  
    }
    // 遍历
    let pg_cnt = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    let end = start + pg_cnt * PAGE_SIZE;
    //println!("mmap  pg_cnt : {} len: {:#x}, start: {:#x}, end: {:#x}", pg_cnt, len, start, end);
    let mut current = start;

    while current < end {
        let current_va = VirtAddr::from(current);
        let current_vpn = current_va.floor();

        if let Some(pte) = page_table.translate(current_vpn) {
            if pte.flags().contains(PTEFlags::V){
                println!("error: reuse this page when {:#x}  {:#x}", current_vpn.0, end);
                return -1;
            }
        }
        current = current + PAGE_SIZE;
    }

    current = start;
    while current < end {
        let current_va = VirtAddr::from(current);
        let current_vpn = current_va.floor();
        
        if let Some(frame) = frame_alloc() {
                        // 重要：清零新分配的内存！
            frame.ppn.get_bytes_array()[..].fill(0);

            page_table.map(current_vpn, frame.ppn, flags);


        } else {
            println!("mmap error ");
            return -1;
            
        }
        
        current = current + PAGE_SIZE;
    }
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");

    if start % PAGE_SIZE != 0 {
        return -1;
    }
    
    if len == 0 {  
        return 0;
    }

    let token = current_user_token();
    let mut page_table = PageTable::from_token(token);
    let pg_cnt = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    println!("unmap  pg_cnt : {}, {}", pg_cnt, len);
    let end = start + pg_cnt * PAGE_SIZE;


    let mut current = start;
    while current < end {
        let vpn = VirtAddr::from(current).floor();
        match page_table.translate(vpn) {
            Some(pte) => {
                if !pte.flags().contains(PTEFlags::V) {
                    println!("error unmap flag V, vpn={:#x}", vpn.0);
                    return -1;
                }
            }
            None => {
                println!("error unmap page, vpn={:#x}", vpn.0);
                return -1;
            }
        }
        current += PAGE_SIZE;
    }

    // 执行取消映射
    current = start;
    while current < end {
        let vpn = VirtAddr::from(current).floor();
        page_table.unmap(vpn);
        current += PAGE_SIZE;
    }
    
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
