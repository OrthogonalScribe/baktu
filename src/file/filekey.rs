use std::{os::unix::prelude::MetadataExt, path::Path};

use libc::statx;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct FileKey {
	pub dev: u64,
	pub ino: u64,
}

impl FileKey {
	// TODO: (S) use best practices for Path args of all functions, see
	// https://nick.groenen.me/notes/rust-path-vs-pathbuf/
	pub fn from_path(path: &Path) -> std::io::Result<FileKey> {
		std::fs::symlink_metadata(path).map(|md| FileKey {
			dev: md.dev(),
			ino: md.ino(),
		})
	}

	pub fn from_statx(stx: &statx) -> FileKey {
		assert!(stx.stx_mask & libc::STATX_INO != 0);
		FileKey {
			dev: libc::makedev(stx.stx_dev_major, stx.stx_dev_minor),
			ino: stx.stx_ino,
		}
	}
}
