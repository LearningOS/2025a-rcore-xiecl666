//! Process management syscalls
//!
use alloc::sync::Arc;
use crate::task::TaskControlBlock;
use crate::timer::get_time_us;
use core::slice;
use crate::mm::translated_byte_buffer;
use core::mem::size_of;
use crate::mm::VirtAddr;
use crate::mm::VirtPageNum;
use crate::mm::MapPermission;
use crate::task::PROCESSOR;
use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
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
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
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
    let us = get_time_us();
	//通过mangger得到*ts的逻辑页
	//判断逻辑页所在段
	//判断段属性
	//copy data
	unsafe {
		let cur_task = PROCESSOR.exclusive_access().current().unwrap();
		let task=cur_task.inner_exclusive_access();
		let mut tmp=translated_byte_buffer(task.get_user_token(),_ts as *const u8,size_of::<TimeVal>());
        let time_val= TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
		let time_ptr = &time_val as *const TimeVal;
		let byte_ptr = time_ptr as *const u8;
		let src_slice = slice::from_raw_parts(byte_ptr, size_of::<TimeVal>());
		let mut offset = 0;
        for buf in tmp.iter_mut() {
            let len = buf.len();
            if offset + len > src_slice.len() {
                // 最后一次复制可能不满
                let remain = src_slice.len() - offset;
                buf[..remain].copy_from_slice(&src_slice[offset..offset+remain]);
                break;
            } else {
                buf.copy_from_slice(&src_slice[offset..offset+len]);
                offset += len;
            }
        }
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel[{}]: sys_mmap ",current_task().unwrap().pid.0);
	//找到任务的控制快
    let cur_task = PROCESSOR.exclusive_access().current().unwrap();
	let mut task=cur_task.inner_exclusive_access();
	let mem_set=&mut task.memory_set;
	//检测参数是否合法
	let s=VirtAddr::from(_start).floor();
	let e=VirtAddr::from(_start+_len).ceil();
	if !VirtAddr::from(_start).aligned(){
		return -1;
	}
	else if _port & !0x7 != 0{
		return -1;
	}
	else if _port & 0x7 == 0{
		return -1;
	}
	else{
		for vpagenum in s.0..e.0{
			match mem_set.translate(VirtPageNum(vpagenum)){
				Some(a)=>{
					if a.is_valid(){
						return -1;
					}
				},
				None=>{
					continue;
				}
			}
		}
		//mem-set的map_one
		// for vpagenum in s.0..e.0{
		// 	if let Some(frame) = frame_alloc(){
		// 		info!("alloc page:{}",frame.ppn.0);
		// 		mem_set.page_table.map(VirtPageNum(vpagenum),frame.ppn,PTEFlags::from_bits_truncate(((_port & 0x7) << 1) as u8)|PTEFlags::U);
		// 	}
		// 	else{
		// 		return -1;
		// 	}
		// }
		mem_set.insert_framed_area(VirtAddr::from(_start), VirtAddr::from(_start+_len), MapPermission::from_bits_truncate(((_port & 0x7) << 1) as u8)|MapPermission::U);
		//创建页表项，加入到当前任务中的
		return 0;
	}
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel [{}]: sys_munmap!",current_task().unwrap().pid.0);
    let cur_task = PROCESSOR.exclusive_access().current().unwrap();
	let mut task=cur_task.inner_exclusive_access();
	let mem_set=&mut task.memory_set;
	//检测参数是否合法
	let s=VirtAddr::from(_start).floor();
	let e=VirtAddr::from(_start+_len).ceil();
	if !VirtAddr::from(_start).aligned(){
		return -1;
	}
	else{
		for vpagenum in s.0..e.0{
			if let Some(a)=mem_set.translate(VirtPageNum(vpagenum)){
				if !a.is_valid(){
					return -1;
				}
			}
		}
		for vpagenum in s.0..e.0{
			//let frame = frame_alloc().unwrap();
			if let Some(a)=mem_set.translate(VirtPageNum(vpagenum)){
				if a.is_valid(){
					// let phy_pagenum=a.ppn();
					// frame_dealloc(phy_pagenum);
					mem_set.page_table.unmap(VirtPageNum(vpagenum));
				}
				
			}	
		}
		//创建页表项，加入到当前任务中的
		return 0;
	}
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
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // add new task to scheduler
	let token = current_user_token();
    let path = translated_str(token, _path);
	let cur_task=current_task().unwrap();
	let cur_inner=&mut cur_task.inner_exclusive_access();
	if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let new_task=Arc::new(TaskControlBlock::new(all_data.as_slice()));
		let tmp=new_task.getpid();
		cur_inner.children.push(new_task.clone());
		add_task(new_task);
        tmp as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
	if _prio<2{
		return -1;
	}
    let cur_task = PROCESSOR.exclusive_access().current().unwrap();
	let mut task=cur_task.inner_exclusive_access();
	task.priority=_prio as usize;
	task.priority as isize
}
