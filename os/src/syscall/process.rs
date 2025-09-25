//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_us;
use crate::task::TASK_MANAGER;
use crate::mm::translated_byte_buffer;
use core::mem::size_of;
use crate::mm::VirtAddr;
use crate::mm::VirtPageNum;
use crate::mm::{MapPermission,frame_dealloc};
use crate::mm::PTEFlags;

use core::slice;
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
    let us = get_time_us();
	//通过mangger得到*ts的逻辑页
	//判断逻辑页所在段
	//判断段属性
	//copy data
	unsafe {
		let inner = TASK_MANAGER.inner.exclusive_access();
		let current_task_id = inner.current_task;
		let cur_task=&inner.tasks[current_task_id];
		let mut tmp=translated_byte_buffer(cur_task.get_user_token(),_ts as *const u8,size_of::<TimeVal>());
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

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    unsafe{
		let inner = TASK_MANAGER.inner.exclusive_access();
		let current_task_id = inner.current_task;
		let cur_task=&inner.tasks[current_task_id];
		let vir_pagenum=VirtAddr::from(_id).floor();
		//let vir_offset=VirtAddr::from(_id).page_offset();
		match _trace_request{
			0=>{
				let table=&cur_task.memory_set;
				let tmp=table.translate(vir_pagenum);
				if let Some(a)=tmp{
					if a.is_valid()&&a.readable()&&a.flags().contains(PTEFlags::U){
						return translated_byte_buffer(cur_task.get_user_token(),_id as *const u8,1)[0][0] as isize;
					}
				}
				-1
			},
			1=>{
				let table=&cur_task.memory_set;
				let tmp=table.translate(vir_pagenum);
				if let Some(a)=tmp{
					if a.is_valid()&&a.writable()&&a.flags().contains(PTEFlags::U){
						let tmp=translated_byte_buffer(cur_task.get_user_token(),_id as *const u8,1)[0].as_mut_ptr() as *mut u8;
						*tmp=_data as u8;
						return 0;
					}
				}
				-1
			},
			2=>{
				{
					return cur_task.task_trace[_id] as isize;
				}
			},
			_=>{
				error!("kernel: sys_trace _trace_request should not be{}",_trace_request);
				-1
			}
		}
	}
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
	//找到任务的控制快
    let mut inner = TASK_MANAGER.inner.exclusive_access();
	let current_task_id = inner.current_task;
	let cur_task=&mut inner.tasks[current_task_id];
	let mem_set=&mut cur_task.memory_set;
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
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let mut inner = TASK_MANAGER.inner.exclusive_access();
	let current_task_id = inner.current_task;
	let cur_task=&mut inner.tasks[current_task_id];
	let mem_set=&mut cur_task.memory_set;
	//检测参数是否合法
	let s=VirtAddr::from(_start).floor();
	let e=VirtAddr::from(_start+_len).ceil();
	if !VirtAddr::from(_start).aligned(){
		return -1;
	}
	else{
		for vpagenum in s.0..e.0{
			//let frame = frame_alloc().unwrap();
			if let Some(a)=mem_set.translate(VirtPageNum(vpagenum)){
				if a.is_valid(){
					let phy_pagenum=a.ppn();
					frame_dealloc(phy_pagenum);
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
