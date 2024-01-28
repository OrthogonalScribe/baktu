use std::{
	error::Error,
	ffi::OsString,
	fs::File,
	io::{self, BufRead, BufReader},
	os::unix::prelude::OsStringExt,
	path::{Path, PathBuf},
};

use blake3::{Hash, HexError};

#[derive(Debug)]
pub struct MetaFile(pub PathBuf);
#[derive(Debug)]
pub struct Record(pub Vec<Line>);

pub mod line {
	// Update appropriate doc/repositories/<version>/index.md if you change these
	pub const IS_DEDUPLICATED: &[u8] = b"is-deduplicated";
	// Commented out until we start using it. Added so we don't forget to update the docs if we
	// 	decide on some other representation.
	// pub const PFX_END_MARKER: &[u8] = b"same-since";
	pub const PFX_NAME: &[u8] = b"name";
	pub const PFX_HASH: &[u8] = b"b3sum";
}
#[derive(Debug)]
pub struct Line(pub Vec<u8>);
impl Line {
	fn parse<P: AsRef<Path>>(&self, meta_file_path: P) -> Result<ParsedLine, Box<dyn Error>> {
		// obeying clippy's lint here would result in less obvious code structure
		#[allow(clippy::collapsible_else_if)]
		if self.0.starts_with(line::PFX_HASH) {
			Ok(ParsedLine::Hash(
				self.0[line::PFX_HASH.len() + 1..].to_vec(),
			))
		} else {
			if self.0.starts_with(line::PFX_NAME) {
				let rel_path = OsString::from_vec(self.0[line::PFX_NAME.len() + 1..].to_vec());
				let meta_parent = meta_file_path
					.as_ref()
					.parent()
					.expect("meta path must have parent");
				Ok(ParsedLine::Path(meta_parent.join(rel_path)))
			} else {
				if self.0 == line::IS_DEDUPLICATED {
					Ok(ParsedLine::IsDeduplicated)
				} else {
					unimplemented!("todo other variants");
				}
			}
		}
	}
}

#[derive(Debug, PartialEq)]
enum ParsedLine {
	Path(PathBuf),
	Hash(Vec<u8>),
	IsDeduplicated,
}

impl MetaFile {
	pub fn records(&self) -> io::Result<Vec<Record>> {
		let mut recs: Vec<Record> = Vec::new();
		let mut rec: Vec<Line> = Vec::new();
		const SEPARATOR: &[u8] = b"--";
		for line in BufReader::new(File::open(&self.0)?).split(b'\n') {
			let line = line?;
			if line == SEPARATOR {
				recs.push(Record(rec));
				rec = Vec::new();
			} else {
				rec.push(Line(line));
			}
		}
		Ok(recs)
	}
}

impl Record {
	/// Returns a record's hash and path, if the record is not a deduplicated file
	// TODO: (M) consider if we want to hex-decode the hash ASAP or treat it as opaque data
	//	+decode: half the size, reduce risk of mistyping if we're not strict with our types in the
	//		code base
	//	+opaque: more flexibility if representation changes
	pub fn get_hash_path_opt<P: AsRef<Path>>(
		&self,
		meta_file_path: P,
	) -> Result<Option<(Hash, PathBuf)>, HexError> {
		let mut hash: Option<Hash> = None;
		let mut path: Option<PathBuf> = None;

		for line in &self.0 {
			match line.parse(&meta_file_path) {
				Ok(ParsedLine::IsDeduplicated) => return Ok(None),
				Ok(ParsedLine::Path(p)) => path = Some(p),
				Ok(ParsedLine::Hash(hex_str)) => hash = Some(Hash::from_hex(hex_str)?),
				_ => {}
			}
		}

		Ok(hash.and_then(|h| path.map(|p| (h, p))))
	}
}

#[cfg(test)]
mod test {
	use super::*;

	// TODO: (S) see if we can use test fixtures, etc. here
	const TEST_META_PATH: &str = "/some valid/unix/tree path/.baktu.meta.brj";
	const TEST_NAME: &[u8] = b"Sales Report Jul-1.202X (final).ver2.FINAL!!.docx         .exe";

	fn expected_path() -> PathBuf {
		PathBuf::from(TEST_META_PATH)
			.parent()
			.unwrap()
			.join(OsString::from_vec(TEST_NAME.to_vec()))
	}

	fn line_name() -> Line {
		Line([line::PFX_NAME, b" ", TEST_NAME].concat())
	}

	// b3sum of file containing b"foobar\n"
	const TEST_B3SUM: &[u8] = b"534659321d2eea6b13aea4f4c94c3b4f624622295da31506722b47a8eb9d726c";

	fn line_hash() -> Line {
		Line([line::PFX_HASH, b" ", TEST_B3SUM].concat())
	}

	// TODO: (S) learn best practices for test code and Result types - unwrap(), ?, or otherwise

	mod line_parse {
		use super::*;

		#[test]
		fn hash() {
			assert_eq!(
				ParsedLine::Hash(TEST_B3SUM.to_vec()),
				line_hash().parse(TEST_META_PATH).unwrap()
			)
		}

		#[test]
		fn path() {
			assert_eq!(
				ParsedLine::Path(expected_path()),
				line_name().parse(TEST_META_PATH).unwrap()
			)
		}
	}

	mod record {
		use super::*;

		#[test]
		fn get_path_hash_opt() {
			let record = Record(vec![line_name(), line_hash()]);
			let expected_hash = Hash::from_hex(TEST_B3SUM).unwrap();

			assert_eq!(
				record.get_hash_path_opt(TEST_META_PATH).unwrap(),
				Some((expected_hash, expected_path()))
			);
		}

		#[test]
		fn get_path_hash_opt_deduplicated() {
			let record = Record(vec![
				line_name(),
				Line(line::IS_DEDUPLICATED.to_vec()),
				line_hash(),
			]);

			assert_eq!(record.get_hash_path_opt(TEST_META_PATH).unwrap(), None);
		}
	}
}
