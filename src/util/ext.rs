use std::{
	ffi::OsString,
	fs::File,
	io::{self, Read},
	os::unix::prelude::{OsStrExt, OsStringExt},
	path::{Path, PathBuf},
};

pub trait PathExt {
	fn read_exact(&self, bytes: usize) -> io::Result<Vec<u8>>;
	fn content_starts_with(&self, prefix: &[u8]) -> bool;
	fn tilde_expand(&self) -> PathBuf;
}

impl PathExt for Path {
	fn read_exact(&self, bytes: usize) -> io::Result<Vec<u8>> {
		let mut buf = vec![0u8; bytes];
		File::open(self)?.read_exact(&mut buf)?;
		Ok(buf)
	}

	fn content_starts_with(&self, prefix: &[u8]) -> bool {
		self.read_exact(prefix.len())
			.is_ok_and(|data| data == prefix)
	}

	fn tilde_expand(&self) -> PathBuf {
		OsString::from_vec(tilde_expand::tilde_expand(self.as_os_str().as_bytes())).into()
	}
}
