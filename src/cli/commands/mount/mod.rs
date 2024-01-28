mod fs_impl;

use std::{error::Error, path::PathBuf};
// use std::ffi::OsString;

use exitcode::USAGE;
use log::info;

// TODO: (S) fix circular deps
use crate::cli::{die, repo_snap_or_die};

// use fs_impl::BaktuFS_TODO;

pub fn exec(mount_point: PathBuf) -> Result<(), Box<dyn Error>> {
	if !mount_point.is_dir() {
		die(
			USAGE,
			&format!("{mount_point:?} is not an existing directory"),
		)
	}

	let snap_data_dir = repo_snap_or_die()?.data_dir();

	info!("mounting {snap_data_dir:?} in {mount_point:?} via FUSE");

	unimplemented!("mount actual baktu snapshot data dir");

	// // Haven't found the canonical list of options yet. [1] lists some under "Other options", says
	// //	passing '-h' lists them all.
	// //	[1] https://www.cs.hmc.edu/~geoff/classes/hmc.cs135.201109/homework/fuse/fuse_doc.html
	// let opts = vec![
	// 	// - single-threaded mode:
	// 	OsString::from("-s"),
	// 	// - run in foreground:
	// 	OsString::from("-f"),
	// 	// - enable debugging output (implies -f):
	// 	OsString::from("-d"),
	// 	// XXX: Throws "fuse: unknown option `volname=hello_world'" at runtime. System has both
	// 	//  fuse 2 and 3.
	// 	// OsString::from("-o"),
	// 	// OsString::from("volname=hello_world"),
	// 	// - mount as read-only:
	// 	OsString::from("-o"),
	// 	OsString::from("ro"),
	// ];

	// // TODO: (S) implement all ops as stubs, printing their invocation, to see what we still might
	// //	need to implement, as recommended in [1]
	// // static mut + unsafe init construct taken from [3]
	// //	[3] https://github.com/carlosgaldino/gotenksfs/blob/9768a65e9cfd911b80fc50009f0e6671a11361ee/src/mount.rs#L5
	// static mut FS: BaktuFS_TODO = BaktuFS_TODO {
	// 	snap_data_dir: None,
	// };
	// unsafe { FS = BaktuFS_TODO::new(snap_data_dir) }

	// // TODO: (S) appropriate fuse logging, probably via fuse_set_log_func [2].
	// //	[2] https://github.com/libfuse/libfuse/blob/master/lib/fuse_log.c
	// let res = unsafe {
	// 	fuse_rs::mount(
	// 		std::env::args_os().next().unwrap(),
	// 		mount_point,
	// 		&mut FS,
	// 		opts,
	// 	)
	// };

	// // Currently never reached if process is interrupted via ^C:
	// info!("fuse_rs::mount done");

	// match res {
	// 	Ok(_) => {
	// 		info!("mount subcommand exiting successfully");
	// 		Ok(())
	// 	}
	// 	Err(e) => die(exitcode::SOFTWARE, &format!("fuse error: {e:?}")),
	// }
}
