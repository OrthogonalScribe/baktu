use std::{
	fs::File,
	io::{self, BufRead, BufReader, Write},
	path::Path,
};

// TODO: (S) use BufReader.split(sep)
/// Reads `path`, splits by `sep` and returns a vector of byte vectors
pub fn vec_from_file<P: AsRef<Path>>(path: P, sep: u8) -> io::Result<Vec<Vec<u8>>> {
	let file = File::open(path)?;
	let mut result: Vec<Vec<u8>> = Vec::new();
	let mut reader = BufReader::new(file);
	loop {
		let mut entry: Vec<u8> = Vec::new();
		let bytes_read = reader
			.read_until(sep, &mut entry)
			.expect("should be able to read all bytes in the file");

		if bytes_read == 0 {
			break Ok(result);
		}

		if entry.last().is_some_and(|b| *b == 0) {
			entry.pop(); // drop trailing separator byte
		};

		result.push(entry);
	}
}

/// Overwrites `file` with `xs`, separated by `sep`
pub fn vec_to_file<P: AsRef<Path>>(file: P, sep: u8, xs: Vec<Vec<u8>>) -> io::Result<()> {
	let mut file = std::fs::OpenOptions::new()
		.write(true)
		.truncate(true)
		.open(file)?;
	for x in xs {
		file.write_all(&x)?;
		file.write_all(&[sep])?;
	}
	Ok(())
}
