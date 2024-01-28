use std::{
	io::{self, Write},
	path::{Path, PathBuf},
};

use crate::cli::die;

use super::{dsv, ext::PathExt};

pub const SEP: u8 = 0u8;

pub fn append(file_path: &PathBuf, entry: &[u8]) -> io::Result<()> {
	let mut file = std::fs::OpenOptions::new()
		.append(true)
		.open(file_path.tilde_expand())?;

	file.write_all(entry)?;
	file.write_all(&[SEP])
}

pub fn filter_not<P: AsRef<Path>>(file: P, entry: &[u8]) -> io::Result<()> {
	let entries: Vec<Vec<u8>> = dsv::vec_from_file(&file, SEP)?;

	if !entries.contains(&Vec::from(entry)) {
		die(
			exitcode::USAGE,
			// TODO: (S) visualize the entry in a more user-friendly way
			&format!("{entry:?} not found in {:?}", file.as_ref()),
		);
	}

	dsv::vec_to_file(
		file,
		SEP,
		entries.into_iter().filter(|x| x != entry).collect(),
	)
}
