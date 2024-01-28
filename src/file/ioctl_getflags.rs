use std::{error::Error, fs::OpenOptions, io::Write, os::fd::AsRawFd};

pub fn dump(path: &std::path::Path, sink: &mut Box<dyn Write>) -> Result<(), Box<dyn Error>> {
	let flags = {
		// auto-closed (ignoring errors) by Drop impl
		let file = OpenOptions::new().read(true).open(path)?;

		let fd = file.as_raw_fd();
		let mut flags: std::os::raw::c_long = 0;
		let ret = unsafe { ioctls::fs_ioc_getflags(fd, &mut flags) };
		assert!(ret == 0);
		flags
	};

	use linux_raw_sys::general::*;

	write!(sink, "lsattr ")?;
	for (ch, flag) in [
		('A', FS_NOATIME_FL),
		('C', FS_NOCOW_FL),
		('D', FS_DIRSYNC_FL),
		('E', FS_ENCRYPT_FL),
		('F', FS_CASEFOLD_FL),
		('I', FS_INDEX_FL),
		('N', FS_INLINE_DATA_FL),
		('P', FS_PROJINHERIT_FL),
		('S', FS_SYNC_FL),
		('T', FS_TOPDIR_FL),
		('V', FS_VERITY_FL),
		('a', FS_APPEND_FL),
		('c', FS_COMPR_FL),
		('d', FS_NODUMP_FL),
		('e', FS_EXTENT_FL),
		('i', FS_IMMUTABLE_FL),
		('j', FS_JOURNAL_DATA_FL),
		('m', FS_NOCOMP_FL),
		('s', FS_SECRM_FL),
		('t', FS_NOTAIL_FL),
		('u', FS_UNRM_FL),
		('x', FS_DAX_FL),
	] {
		if flags & flag as i64 != 0 {
			write!(sink, "{}", ch)?;
		}
	}
	writeln!(sink)?;
	Ok(())
}
