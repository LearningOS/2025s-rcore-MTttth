//! Process management syscalls
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::config::*;
use crate::timer::get_time_ms;
use crate::{
    loader::get_app_data_by_name,
    mm::{
        parse_prot_to_flags, translated_refmut, translated_str, PageTable,
        SimpleRange, VirtAddr, VirtPageNum,
    },
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, make_new_map_area,
        suspend_current_and_run_next
    },
};
pub const USER_VADDR_MAX: usize = (1 << 39) - 1;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!(
        "kernel::pid[{}] sys_waitpid [{}]",
        current_task().unwrap().pid.0,
        pid
    );
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
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
    let token = current_user_token();
    // debug!("token is {:?}", token);

    let page_table = PageTable::from_token(token);

    // 虚拟地址
    let vaddr = VirtAddr::from(_ts as usize);
    // debug!("v_addr is {:?}", vaddr);

    let vpn = VirtPageNum::from(vaddr.floor());
    // debug!("vpn is {:?}", vpn);

    // 映射为物理地址
    if let Some(pte) = page_table.translate(vpn) {
        let paddr = pte.ppn().0 << PAGE_SIZE_BITS | vaddr.page_offset(); // 物理地址 = 页基地址 + 页内偏移
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

/// YOUR JOB: Implement mmap.
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
    let token = current_user_token();
    debug!("[sys_mmap] token is {:#x}", token);
    let page_table = PageTable::from_token(token);
    for vpn in vpn_range.clone() {
        if let Some(pte) = page_table.translate(vpn) {
            if pte.is_valid() {
                warn!(
                    "[sys_mmap] vpn {:#x} already mapped at ppn {:#x}",
                    vpn.0,
                    pte.ppn().0
                );
                return -1;
            }
        }
    }
    make_new_map_area(start_vaddr, end_vaddr, flags);
    // 映射完成
    0
}

/// YOUR JOB: Implement munmap.
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
    let token = current_user_token();
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
        // 以下语句肯可能会导致 ummap 与 FrameAllocator 冲突, (考虑生命周期结束自动释放)
        //frame_dealloc(pte.ppn());
        page_table.unmap(vpn);
    }

    return 0;
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
/// Spawn a new process from ELF by path in user space
/// Return child pid in parent, or -1 on failure
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let current_task = current_task().unwrap();
        let new_task = current_task.spawn(data);
        let new_pid = new_task.pid.0;
        // modify trap context of new_task, because it returns immediately after switching
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        // we do not have to move to next instruction since we have done it before
        // for child process, fork returns 0
        trap_cx.x[10] = 0;
        // add new task to scheduler
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let current_task = current_task().unwrap();
    if _prio >= 2 {
        current_task.inner_exclusive_access().prio = _prio as isize;
        debug!(
            "[sys_set_priority] pid[{}] set priority to {}",
            current_task.getpid(),
            _prio
        );
        return _prio;
    }
    -1
}
