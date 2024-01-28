use std::path::{Path, PathBuf};

// TODO: (P) switch to std::sync::Lazy, see
//   https://rust-lang.github.io/rfcs/2788-standard-lazy-types.html
//   https://github.com/rust-lang/rust/pull/105587

use log::debug;
use once_cell::sync::Lazy;

use crate::util::ext::PathExt;

// Use macro to work around include_str not accepting string constants
macro_rules! NAME_MACRO {
	() => {
		"BAKTU_REPO.TAG"
	};
}

// Avoid historical macro scoping warts
pub static NAME: &str = NAME_MACRO!();

static DATA: &str = include_str!(concat!("../../templates/", NAME_MACRO!()));

// Using Lazy as Option::expect is not const yet: https://github.com/rust-lang/rust/issues/67441
static DATA_PREFIX: Lazy<&[u8]> = Lazy::new(|| {
	DATA.lines()
		.next()
		.expect("tag file should have at least one line")
		.as_bytes()
});

pub fn is_valid(path: PathBuf) -> bool {
	path.content_starts_with(&DATA_PREFIX)
}

pub fn create_in(dir: &Path) -> std::io::Result<()> {
	let tag_file_path = dir.join(NAME);

	debug!("creating tag file {:?}", tag_file_path);
	std::fs::write(tag_file_path, DATA)
}
