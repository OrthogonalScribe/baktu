use std::{ffi::OsStr, fs::File, io::Read, os::unix::prelude::OsStrExt, path::PathBuf};

use super::{meta_file::MetaFile, site};

pub const META_NAME_FNAME: &str = "meta_name.cfg.bin";

#[derive(Debug)]
pub struct Snapshot(pub PathBuf);

impl Snapshot {
	/// Facade for try_from() that can skip checks that don't make sense during a dry run
	pub fn from_dryable(path: PathBuf, dry_run: bool) -> std::io::Result<Self> {
		if dry_run {
			Ok(Self(path))
		} else {
			Self::try_from(path)
		}
	}

	// XXX: sub-optimal situation with partial validity checks here and in try_from(). Ideally we'll
	// want to have clear Snapshot states with clear verification criteria, while avoiding
	// unnecessary validation where pragmatic.
	pub fn is_valid<P: AsRef<std::path::Path>>(path: P) -> bool {
		path.as_ref().is_dir() && path.as_ref().parent().is_some_and(site::is_snapshots_dir)
	}

	pub fn meta_files(&self) -> std::io::Result<impl Iterator<Item = walkdir::Result<MetaFile>>> {
		let name = {
			let mut buffer = Vec::new();
			File::open(self.0.join(META_NAME_FNAME))?.read_to_end(&mut buffer)?;
			OsStr::from_bytes(&buffer).to_owned()
		};

		// TODO: (M) take care of unwrap() below
		Ok(walkdir::WalkDir::new(&self.0)
			.sort_by_file_name()
			.into_iter()
			.filter(move |e| e.as_ref().unwrap().file_name() == name)
			.map(|de_res| de_res.map(|de| MetaFile(de.path().to_path_buf()))))
	}

	pub fn data_dir(&self) -> PathBuf {
		self.0.join("data")
	}
}

impl TryFrom<std::fs::DirEntry> for Snapshot {
	type Error = std::io::Error;

	fn try_from(de: std::fs::DirEntry) -> std::io::Result<Self> {
		if !de.file_type()?.is_dir() {
			// TODO: (P) use [1] if they ever untangle the mess enough to stabilize it
			// [1] https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.NotADirectory
			Err(std::io::Error::from_raw_os_error(libc::ENOTDIR))
		} else {
			Ok(Self(de.path()))
		}
	}
}

impl TryFrom<PathBuf> for Snapshot {
	type Error = std::io::Error;

	fn try_from(path: PathBuf) -> std::io::Result<Self> {
		if !path.is_dir() {
			// TODO: (P) use [1] if they ever untangle the mess enough to stabilize it
			// [1] https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.NotADirectory
			Err(std::io::Error::from_raw_os_error(libc::ENOTDIR))
		} else {
			Ok(Self(path))
		}
	}
}
