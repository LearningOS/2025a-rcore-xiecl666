//! File and filesystem-related syscalls
use crate::fs::{open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};
use crate::fs::ROOT_INODE;
use crate::fs::{OSInode,StatMode};
use alloc::sync::Arc;
use core::mem::size_of;
use core::slice;
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat",
        current_task().unwrap().pid.0
    );
	let file={
		let tck=&current_task().unwrap().clone();
		let inner=tck.inner_exclusive_access();
		inner.fd_table[_fd].as_ref().unwrap().clone()
	};

	let os_inode: Arc<OSInode> = unsafe {
		// 安全条件：确保 file 确实是 Arc<OSInode>
		Arc::from_raw(Arc::into_raw(file) as *const OSInode)
	};
	unsafe {
		let tck=&current_task().unwrap().clone();
		let mut tmp=translated_byte_buffer(tck.get_user_token(),_st as *const u8,size_of::<Stat>());
		let inode_inner=os_inode.inner.exclusive_access();
		let inode=inode_inner.inode.clone();
		//这里不释放，会与read_disk_inode多次上锁
		drop(inode_inner);
		let (nlink,is_dir)=inode.read_disk_inode(|disk_node|{
			(disk_node.nlink,disk_node.is_dir())
		});
		let mode = if is_dir { StatMode::DIR } else { StatMode::FILE };
		let tmp_stat= Stat {
            dev: 0,
    		ino: inode.get_inode_id(),
    		mode: mode,
    		nlink: nlink,
			pad:[0;7]
        };
		let stat_ptr = &tmp_stat as *const Stat;
		let byte_ptr = stat_ptr as *const u8;
		let src_slice = slice::from_raw_parts(byte_ptr, size_of::<Stat>());
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

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_name = translated_str(token, _old_name);
	let new_name = translated_str(token, _new_name);
		// let nstr = CStr::from_ptr(_new_name as *const i8); // CStr 接受 *const i8
    	// let new_name=str::from_utf8(nstr.to_bytes()).ok().unwrap();
		// let ostr = CStr::from_ptr(_old_name as *const i8); // CStr 接受 *const i8
    	// let old_name=str::from_utf8(ostr.to_bytes()).ok().unwrap();
    	//找到两个路径对应的inode
		//可能需要新建一个inode属性使用旧的inode
		if let Some(_)=ROOT_INODE.find(&new_name){
			return -1;
		}
		if new_name==old_name{
			return -1;
		}
		ROOT_INODE.create_link(&old_name, &new_name);
	0
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat",
        current_task().unwrap().pid.0
    );
	let token = current_user_token();
    let name = translated_str(token, _name);
		// let _name = CStr::from_ptr(_name as *const i8); // CStr 接受 *const i8
    	// let name=str::from_utf8(_name.to_bytes()).ok().unwrap();
    	//找到两个路径对应的inode
		//可能需要新建一个inode属性使用旧的inode
		if let None=ROOT_INODE.find(&name){
			return -1;
		}
		ROOT_INODE.delete_link(&name);
	0
}
