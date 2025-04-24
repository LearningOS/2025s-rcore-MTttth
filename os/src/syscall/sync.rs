use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        let mut banker = process.banker_exclusive_access();
        banker.register_resource(id, 1);
        drop(banker);
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        let id = process_inner.mutex_list.len() as isize - 1;
        let mut banker = process.banker_exclusive_access();
        banker.register_resource(id as usize, 1);
        drop(banker);
        id
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        pid, tid
    );

    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let enable = process_inner.enable_deadlock_detect;
    drop(process_inner);
    if enable == true {
        let mut banker = process.banker_exclusive_access();
        banker.increase_need(tid, mutex_id, 1);
        let is_safe = banker.try_allocate(tid, mutex_id, 1);
        drop(banker);
        if !is_safe {
            return -0xdead;
        }
    }
    drop(process);
    mutex.lock();
    0
}

/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        pid, tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    drop(process_inner);
    if enable_deadlock_detect == true {
        let mut banker = process.banker_exclusive_access();
        banker.increase_available(mutex_id, 1);
        drop(banker);
    }
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        pid, tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    drop(process_inner);
    let mut banker = process.banker_exclusive_access();
    banker.register_resource(id, res_count);
    drop(banker);
    drop(process);
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        pid, tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    drop(process_inner);
    if enable_deadlock_detect == true && sem_id != 0 {
        let mut banker = process.banker_exclusive_access();
        banker.increase_available(sem_id, 1);
        drop(banker);
    }
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        pid, tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    drop(process_inner);
    if enable_deadlock_detect == true && sem_id != 0 {
        let mut banker = process.banker_exclusive_access();
        banker.increase_need(tid, sem_id, 1);
        let is_safe = banker.try_allocate(tid, sem_id, 1);
        drop(banker);

        if !is_safe {
            return -0xdead;
        }
    }
    sem.down();
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let status = enabled == true as usize;
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    inner.enable_deadlock_detect = status;
    drop(inner);
    0
}
