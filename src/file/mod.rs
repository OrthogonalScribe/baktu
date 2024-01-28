pub mod filekey;
pub mod ioctl_getflags;
pub mod statx;
pub mod xattrs;

use std::{
	fs::File,
	io::{self, BufReader, Read},
	path::Path,
};

use blake3::Hash;

use crate::util::ext::PathExt;

pub fn is_valid_cachedir_tag(path: &Path) -> bool {
	path.content_starts_with(b"Signature: 8a477f597d28d172789f06886806bc55")
}

/// Calculates BLAKE3 digest of file
pub fn b3sum(file_path: &Path) -> io::Result<Hash> {
	let file = File::open(file_path)?;
	let mut reader = BufReader::new(file);
	let mut hasher = blake3::Hasher::new();
	let mut buffer = vec![0; 65536];
	loop {
		match reader.read(&mut buffer)? {
			0 => return Ok(hasher.finalize()),
			n => {
				hasher.update(&buffer[..n]);
			}
		}
	}
}
