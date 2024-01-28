use std::{
	io::{self, Write},
	mem::MaybeUninit,
	os::unix::prelude::OsStrExt,
	path::Path,
};

use libc::statx;

pub fn get(path: &Path) -> io::Result<libc::statx> {
	// We use libc::statx here, as its statx type is the most up to date, at the cost of lacking
	//	some creature comforts in terms of invocation, arg conversion and error handling.
	//
	// Alternatives:
	//	1. Use nc: nc::statx_t lacks stx_mnt_id and later, so that would require a local clone, and
	//		would ideally be followed by a PR and an checking the change's effect on support for
	//		older kernels and other compatibility issues.
	//	2. Use nix: Lacks statx, so a similar procedure would be required, possibly with a larger
	//		scope on account of adding an entirely new system call too.
	//
	// Discussion on error handling:
	//	https://stackoverflow.com/questions/42772307/how-do-i-handle-errors-from-libc-functions-in-an-idiomatic-rust-manner/42773525#42773525
	let mut result = MaybeUninit::<statx>::zeroed();
	let ret = unsafe {
		libc::statx(
			libc::AT_FDCWD,
			std::ffi::CString::new(path.as_os_str().as_bytes())?.as_ptr(),
			libc::AT_SYMLINK_NOFOLLOW,
			libc::STATX_BASIC_STATS | libc::STATX_BTIME | libc::STATX_MNT_ID | libc::STATX_DIOALIGN,
			result.as_mut_ptr(),
		)
	};
	if ret == 0 {
		Ok(unsafe { result.assume_init() })
	} else {
		Err(io::Error::last_os_error())
	}
}

/// Writes a lossless textual representation of `statx()` data of `path` to `sink`. Explicitly not
/// UTF-8 safe. Does not follow symlinks.
pub fn dump(stx: statx, sink: &mut Box<dyn Write>) -> io::Result<()> {
	writeln!(sink, "blksize {}", stx.stx_blksize)?;

	write!(sink, "attributes")?;
	let mut print_attr = |flag: i32, name: &str| -> std::io::Result<()> {
		if (stx.stx_attributes_mask & flag as u64) != 0 && (stx.stx_attributes & flag as u64) != 0 {
			write!(sink, " {}", name)
		} else {
			Ok(())
		}
	};
	print_attr(libc::STATX_ATTR_COMPRESSED, "compressed")?;
	print_attr(libc::STATX_ATTR_IMMUTABLE, "immutable")?;
	print_attr(libc::STATX_ATTR_APPEND, "append")?;
	print_attr(libc::STATX_ATTR_NODUMP, "nodump")?;
	print_attr(libc::STATX_ATTR_ENCRYPTED, "encrypted")?;
	print_attr(libc::STATX_ATTR_VERITY, "verity")?;
	print_attr(libc::STATX_ATTR_DAX, "dax")?;
	writeln!(sink)?;

	assert!(stx.stx_mask & libc::STATX_NLINK != 0);
	writeln!(sink, "nlink {}", stx.stx_nlink)?;

	assert!(stx.stx_mask & libc::STATX_UID != 0);
	writeln!(sink, "uid {}", stx.stx_uid)?;

	assert!(stx.stx_mask & libc::STATX_GID != 0);
	writeln!(sink, "gid {}", stx.stx_gid)?;

	// ~S_IFMT from https://man7.org/linux/man-pages/man2/statx.2.html
	assert!(stx.stx_mask & libc::STATX_MODE != 0);
	writeln!(sink, "mode {:o}", stx.stx_mode as u32 & !libc::S_IFMT)?;

	assert!(stx.stx_mask & libc::STATX_TYPE != 0);
	match stx.stx_mode as u32 & libc::S_IFMT {
		libc::S_IFIFO => writeln!(sink, "type fifo")?,
		libc::S_IFCHR => writeln!(sink, "type chr")?,
		libc::S_IFDIR => writeln!(sink, "type dir")?,
		libc::S_IFBLK => writeln!(sink, "type blk")?,
		libc::S_IFREG => writeln!(sink, "type reg")?,
		libc::S_IFLNK => writeln!(sink, "type lnk")?,
		libc::S_IFSOCK => writeln!(sink, "type sock")?,
		unknown => writeln!(sink, "type unknown: {}", unknown)?,
	}

	assert!(stx.stx_mask & libc::STATX_INO != 0);
	writeln!(sink, "ino {}", stx.stx_ino)?;

	assert!(stx.stx_mask & libc::STATX_SIZE != 0);
	writeln!(sink, "size {}", stx.stx_size)?;

	assert!(stx.stx_mask & libc::STATX_BLOCKS != 0);
	writeln!(sink, "blocks {}", stx.stx_blocks)?;

	assert!(stx.stx_mask & libc::STATX_ATIME != 0);
	writeln!(
		sink,
		"atime {}.{:09}",
		stx.stx_atime.tv_sec, stx.stx_atime.tv_nsec
	)?;

	assert!(stx.stx_mask & libc::STATX_BTIME != 0);
	writeln!(
		sink,
		"btime {}.{:09}",
		stx.stx_btime.tv_sec, stx.stx_btime.tv_nsec
	)?;

	assert!(stx.stx_mask & libc::STATX_CTIME != 0);
	writeln!(
		sink,
		"ctime {}.{:09}",
		stx.stx_ctime.tv_sec, stx.stx_ctime.tv_nsec
	)?;

	assert!(stx.stx_mask & libc::STATX_MTIME != 0);
	writeln!(
		sink,
		"mtime {}.{:09}",
		stx.stx_mtime.tv_sec, stx.stx_mtime.tv_nsec
	)?;

	assert!(stx.stx_mask & libc::STATX_TYPE != 0);
	match stx.stx_mode as u32 & libc::S_IFMT {
		libc::S_IFCHR | libc::S_IFBLK => {
			writeln!(sink, "rdev_major {}", stx.stx_rdev_major)?;
			writeln!(sink, "rdev_minor {}", stx.stx_rdev_minor)?;
		}
		_ => (),
	};

	writeln!(sink, "dev_major {}", stx.stx_dev_major)?;
	writeln!(sink, "dev_minor {}", stx.stx_dev_minor)?;

	assert!(stx.stx_mask & libc::STATX_MNT_ID != 0);
	writeln!(sink, "mnt_id {}", stx.stx_mnt_id)?;

	if stx.stx_mask & libc::STATX_DIOALIGN != 0 {
		writeln!(sink, "dio_mem_align {}", stx.stx_dio_mem_align)?;
		writeln!(sink, "dio_offset_align {}", stx.stx_dio_offset_align)?;
	}

	Ok(())
}
