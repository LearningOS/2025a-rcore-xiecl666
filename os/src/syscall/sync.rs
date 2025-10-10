use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
//use alloc::vec::Vec;
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
		process_inner.mux_deadlockdectector.available[id]=1;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        let id=process_inner.mutex_list.len() - 1;
		process_inner.mux_deadlockdectector.available[id]=1;
		id as isize
    }
}
///
fn deadlock_detect(tid:usize,mux_id:usize,_type:usize)->bool{
	let process = current_process();
	let process_inner = process.inner_exclusive_access();
	//let n=process_inner.tasks.len();
	//let mut finish:Vec<bool> = (0..n).map(|_| false).collect();
	if process_inner.enable_deadlock_detect==0{
		return true;
	}
	let mut work: [isize; 5];
	let all: [(usize, [usize; 5]); 17];
	let mut need: [(usize, [usize; 5]); 17];
	if _type==0{
		let work=process_inner.mux_deadlockdectector.available.clone();
		if work[mux_id]==0{
			return false;
		}
	}
	// else{
	// 	let work=process_inner.sem_deadlockdectector.available.clone();
	// 	if work[mux_id]==0{
	// 		return false;
	// 	}
	// }
	else{
		work=process_inner.sem_deadlockdectector.available.clone();
		all=process_inner.sem_deadlockdectector.allocations.clone();
		need={
			let mut arr = [(0, [0; 5]); 17];
			for (i, elem) in arr.iter_mut().enumerate() {
				elem.0 = i; // 第一个元素设为1-17
			}
			arr
		};
		let all_tasks=&process_inner.tasks;
		need[tid].1[mux_id]+=1;
		for i in 0..5{
			if i>=process_inner.semaphore_list.len(){
				break;
			}
			if let Some(sem)=&process_inner.semaphore_list[i]{
				let sem_inner=sem.inner.exclusive_access();
				let wait_tasks=&sem_inner.wait_queue;
				for wait_task in wait_tasks{
					for (id,task_opt) in all_tasks.iter().enumerate(){
						if let Some(task) = task_opt {
							// Compare the underlying TaskControlBlock using Arc::ptr_eq
							if Arc::ptr_eq(wait_task, task) {
								need[id].1[i] += 1;
							}
						}
					}
				}
			}	
		}
	}
	let mut finish=[false;17];
	loop {
		let mut found = false;
			
			// Step 2: Find a thread that can be satisfied
		for i in 0..17 {
			if !finish[i] {
				let mut can_allocate = true;
					
					// Check if Need[i] <= Work for all resources
				for j in 0..5 {
					if need[i].1[j] > work[j] as usize {
						can_allocate = false;
						break;
					}
				}
					// Step 3: If found, allocate resources
				if can_allocate {
					found = true;
						
						// Release allocated resources
					for j in 0..5 {
						work[j] += all[i].1[j] as isize;
					}
						
					finish[i] = true;
					break; // Restart search after updating work
				}
			}
		}
			
			// Step 4: If no thread found, break the loop
		if !found {
			break;
		}
	}
	let is_safe = finish.iter().all(|&x| x);

	if !is_safe {
		return false;
	}
	true
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
	let tid=current_task()
		.unwrap()
		.inner_exclusive_access()
		.res
		.as_ref()
		.unwrap()
		.tid;
    let process = current_process();
    if !deadlock_detect(tid,mutex_id,0){
		return -0xDEAD;
	}
	let mut process_inner = process.inner_exclusive_access();
	//加入死锁检测函数
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    if process_inner.mux_deadlockdectector.available[mutex_id]>0
	{
		process_inner.mux_deadlockdectector.allocations[tid].1[mutex_id]+=1;
		process_inner.mux_deadlockdectector.available[mutex_id]-=1;
	}
	drop(process_inner);
    drop(process);
    mutex.lock();
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
	let tid=current_task()
		.unwrap()
		.inner_exclusive_access()
		.res
		.as_ref()
		.unwrap()
		.tid;
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    process_inner.mux_deadlockdectector.allocations[tid].1[mutex_id]-=1;
	process_inner.mux_deadlockdectector.available[mutex_id]+=1;
	drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
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
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.sem_deadlockdectector.available[id]=res_count as isize;
		id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        let id=process_inner.semaphore_list.len() - 1;
		process_inner.sem_deadlockdectector.available[id]=res_count as isize;
		id
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
	let tid=current_task()
		.unwrap()
		.inner_exclusive_access()
		.res
		.as_ref()
		.unwrap()
		.tid;
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    process_inner.sem_deadlockdectector.allocations[tid].1[sem_id]-=1;
	process_inner.sem_deadlockdectector.available[sem_id]+=1;
	drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
	let tid=current_task()
		.unwrap()
		.inner_exclusive_access()
		.res
		.as_ref()
		.unwrap()
		.tid;
    let process = current_process();
    //加入死锁检测函数
	if !deadlock_detect(tid,sem_id,1){
		return -0xDEAD;
	}
	let mut process_inner = process.inner_exclusive_access();
	let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
	if process_inner.sem_deadlockdectector.available[sem_id]>0{
		process_inner.sem_deadlockdectector.allocations[tid].1[sem_id]+=1;
		process_inner.sem_deadlockdectector.available[sem_id]-=1;
	}
	drop(process_inner);
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
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
	if _enabled>1{
		return -1;
	}
	let process = current_process();
	let mut process_inner = process.inner_exclusive_access();
	process_inner.enable_deadlock_detect=_enabled;
    0
}
