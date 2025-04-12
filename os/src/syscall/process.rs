//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next, get_current_task, get_syscall_count},
    timer::get_time_us,
    loader::{read_current_task_bytes, write_current_task_bytes},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            // Handle case where _trace_request is 0
            trace!("Handling trace request READ");
            let app_id = get_current_task();
            let byte = read_current_task_bytes(app_id, _id);
            byte as isize
        }
        1 => {
            // Handle case where _trace_request is 1
            trace!("Handling trace request WRITE");
            let app_id = get_current_task();
            write_current_task_bytes(app_id, _id, _data);
            0
        }
        2 => {
            // Handle case where _trace_request is 2
            trace!("Handling trace request 2");
            let mut count = get_syscall_count(_id).try_into().unwrap();
            if _id == 410 {
                count += 1;
            }
            // println!("syscall id is {}, syscall count: {}\n", _id ,count);
            count
            
        }
        _ => {
            // Handle all other cases
            trace!("Unhandled trace request");
            -1 // Return an error code or default value
        }
    }
}
