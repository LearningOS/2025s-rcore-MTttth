//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, get_current_user_token, get_syscall_count, make_new_map_area};
use crate::timer::get_time_ms;
use crate::mm::{PageTable, VirtAddr, VirtPageNum, parse_prot_to_flags, SimpleRange, frame_dealloc};
use crate::config::*;
use alloc::vec::Vec;
pub const USER_VADDR_MAX: usize = (1 << 39) - 1;


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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    // 当前时间
    let ms = get_time_ms();
    let us = ms * 1000;

    // debug!("_ts is {:?}", _ts);

    // 当前用户 token 和页表
    let token = get_current_user_token();
    // debug!("token is {:?}", token);

    let page_table = PageTable::from_token(token);

    // 虚拟地址
    let vaddr = VirtAddr::from(_ts as usize);
    // debug!("v_addr is {:?}", vaddr);

    let vpn = VirtPageNum::from(vaddr.floor()); 
    // debug!("vpn is {:?}", vpn);

    // 映射为物理地址
    if let Some(ppn) = page_table.translate(vpn) {
        let paddr = ppn.ppn().0 << PAGE_SIZE_BITS | vaddr.page_offset(); // 物理地址 = 页基地址 + 页内偏移
        // debug!("ppn base = 0x{:x}, page_offset = 0x{:x}, paddr = 0x{:x}", ppn.ppn().0, vaddr.page_offset(), paddr);

        unsafe {
            *(paddr as *mut TimeVal) = TimeVal {
                sec: (us / 1_000_000) as usize,
                usec: (us % 1_000_000) as usize,
            };
        }
        0
    } else {
        warn!("[sys_get_time] invalid user pointer {:?}", vaddr);
        -1
    }
}


/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            match _id > USER_VADDR_MAX {
                true => {
                    warn!("[sys_trace] invalid user pointer {:#x}", _id);
                    return -1;
                }
                false => {}
            }
            // 当前用户 token 和页表
            let token = get_current_user_token();
            // debug!("token is {:?}", token);

            let page_table = PageTable::from_token(token);

            // 虚拟地址
            let vaddr = VirtAddr::from(_id as usize);
            debug!("[sys_trace] v_addr is {:#x}", vaddr.0);

            let vpn = VirtPageNum::from(vaddr.floor()); 
            debug!("[sys_trace] vpn is {:#x}", vpn.0);

            // 映射为物理地址
            if let Some(page_table_entry) = page_table.translate(vpn) {
                match page_table_entry.readable() && page_table_entry.is_valid(){
                    true => {
                        let paddr = page_table_entry.ppn().0 << PAGE_SIZE_BITS | vaddr.page_offset(); // 物理地址 = 页基地址 + 页内偏移
                        debug!("[sys_trace] ppn = 0x{:#x}, page_offset = 0x{:#x}, paddr = 0x{:#x}", page_table_entry.ppn().0, vaddr.page_offset(), paddr);
                        
                        unsafe {
                            let ptr = paddr as *const u8;
                            let data = ptr.read_volatile();
                            return data as isize;
                        }
                    }
                    false => {
                        warn!("[sys_trace] invalid user pointer {:#x}", vaddr.0);
                        return -1;
                    }
                }
            } else {
                warn!("[sys_trace] failed to translate vpn {:#x}", vpn.0);
                return -1;
            }
        }
        1 => {
            match _id > USER_VADDR_MAX {
                true => {
                    warn!("[sys_trace] invalid user pointer {:#x}", _id);
                    return -1;
                }
                false => {}
            }
            // 当前用户 token 和页表
            let token = get_current_user_token();
            // debug!("token is {:?}", token);

            let page_table = PageTable::from_token(token);

            // 虚拟地址
            let vaddr = VirtAddr::from(_id as usize);
            // debug!("v_addr is {:?}", vaddr);

            let vpn = VirtPageNum::from(vaddr.floor()); 
            // debug!("vpn is {:?}", vpn);

            // 映射为物理地址
            if let Some(page_table_entry) = page_table.translate(vpn) {
                match page_table_entry.readable() && page_table_entry.writable(){
                    true => {
                        let paddr = page_table_entry.ppn().0 << PAGE_SIZE_BITS | vaddr.page_offset(); // 物理地址 = 页基地址 + 页内偏移
                        // debug!("ppn base = 0x{:x}, page_offset = 0x{:x}, paddr = 0x{:x}", ppn.ppn().0, vaddr.page_offset(), paddr);
                        
                        unsafe {
                            let ptr = paddr as *mut u8;
                            ptr.write_volatile(_data as u8);
                        }
                        return 0;
                    }
                    false => {
                        warn!("[sys_trace] invalid user pointer {:#x}", vaddr.0);
                        return -1;
                    }
                }
            } else {
                warn!("[sys_trace] failed to translate vpn {:#x}", vpn.0);
                return -1;
            }
        }
        2 => {
            let mut count = get_syscall_count(_id).try_into().unwrap();
            if _id == 410 {
                count += 1;
            }
            debug!("[sys_trace] syscall id is {:?}, syscall count: {:?}\n", _id ,count);
            count
        }
        _ => {
            -1
        }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let start_vaddr = VirtAddr::from(_start);
    let end_vaddr = VirtAddr::from(_start + _len);
    let start_vpn = VirtPageNum::from(start_vaddr.floor());
    let end_vpn = VirtAddr::from(end_vaddr).ceil();
    let vpn_range = SimpleRange::new(start_vpn, end_vpn);
    debug!("[sys_mmap] start_vaddr is {:#x}, end_vaddr is {:#x}, start_vpn is {:#x}, end_vpn is {:#x}, length is {:?}", start_vaddr.0, end_vaddr.0, start_vpn.0, end_vpn.0, _len);

    let flags = match parse_prot_to_flags(_port) {
        Some(f) => f,
        None => {
            warn!("[sys_mmap] invalid prot {:#x}", _port);
            return -1;
        }
    };
    match start_vaddr.aligned() {
        true => {}
        false => {
            warn!("[sys_mmap] not aligned {:#x}", start_vaddr.0);
            return -1;
        }
    }
    match _len < USER_VADDR_MAX {
        true => {}
        false => {
            warn!("[sys_mmap] invalid length {:?}", _len);
            return -1;
        }
    }
    // 当前用户 token 和页表
    let token = get_current_user_token();
    debug!("[sys_mmap] token is {:#x}", token);
    let page_table = PageTable::from_token(token);
    for vpn in vpn_range.clone() {
        if let Some(pte) = page_table.translate(vpn) {
            if pte.is_valid() {
                warn!("[sys_mmap] vpn {:#x} already mapped at ppn {:#x}", vpn.0, pte.ppn().0);
                return -1;
            }
        }
    }
    make_new_map_area(start_vaddr, end_vaddr, flags);
    // 映射完成
    0

}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let start_vaddr = VirtAddr::from(_start);
    let end_vaddr = VirtAddr::from(_start + _len);
    let start_vpn = VirtPageNum::from(start_vaddr.floor());
    let end_vpn = VirtAddr::from(end_vaddr).ceil();
    let vpn_range = SimpleRange::new(start_vpn, end_vpn);
    debug!("[sys_munmap] start_vaddr is {:#x}, end_vaddr is {:#x}, start_vpn is {:#x}, end_vpn is {:#x}, length is {:?}", start_vaddr.0, end_vaddr.0, start_vpn.0, end_vpn.0, _len);

    match start_vaddr.aligned() {
        true => {}
        false => {
            warn!("[sys_munmap] not aligned {:#x}", start_vaddr.0);
            return -1;
        }
    }
    match _len < USER_VADDR_MAX {
        true => {}
        false => {
            warn!("[sys_munmap] invalid length {:?}", _len);
            return -1;
        }
    }
    // 当前用户 token 和页表
    let token = get_current_user_token();
    debug!("[sys_munmap] token is {:#x}", token);
    let mut page_table = PageTable::from_token(token);

    let mut pte_list = Vec::new();
    for vpn in vpn_range.clone() {
        if let Some(pte) = page_table.translate(vpn) {
            if pte.is_valid() {
                pte_list.push((vpn, pte));
            } else {
                warn!("[sys_munmap] page {:#x} not mapped", vpn.0);
                return -1;
            }
        } else {
            warn!("[sys_munmap] page {:#x} not mapped", vpn.0);
            return -1;
        }
    }

    // 现在再进行 unmap
    for (vpn, pte) in pte_list {
        debug!("[sys_munmap]--------------- drop vpn is {:#x}", vpn.0);
        debug!("[sys_munmap]--------------- drop ppn is {:#x}", pte.ppn().0);
        frame_dealloc(pte.ppn());
        page_table.unmap(vpn);
        debug!("[sys_munmap]--------------- drop pte is {:#x}", pte.ppn().0);
    }

    return 0;
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
