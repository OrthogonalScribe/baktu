use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn version() {
	Command::cargo_bin("baktu")
		.unwrap()
		.arg("-V")
		.assert()
		.success()
		.stdout(format!("baktu {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn init_nonempty() {
	let temp = assert_fs::TempDir::new().unwrap();

	temp.child("foo.txt").write_str("foo bar").unwrap();

	Command::cargo_bin("baktu")
		.unwrap()
		.current_dir(&temp)
		.arg("init")
		.assert()
		.failure()
		.stdout("")
		.stderr(predicate::str::contains(
			"current directory is not empty, exiting",
		));
}

#[test]
fn init_succeeds() {
	let temp = assert_fs::TempDir::new().unwrap();

	Command::cargo_bin("baktu")
		.unwrap()
		.current_dir(&temp)
		.arg("init")
		.assert()
		.success()
		.stdout("")
		.stderr("");

	// Ensure the repository has the correct content:

	// 1. Directory structure
	Command::new("tree")
		.current_dir(&temp)
		.arg("-aFN") // all files, show type, show non-printable characters as-is
		.arg("--noreport") // GH runner `tree` does not count . as a directory
		.assert()
		.success()
		.stdout(
			"./\n\
			├── BAKTU_REPO.TAG\n\
			└── sites/\n",
		);

	// 2. File content
	// We're explicitly using literal strings here instead of tag_file::{NAME,DATA} to require two
	// places to be changed when doing repository format changes.
	temp.child("BAKTU_REPO.TAG")
		.assert("baktu repository version 1\n");
}
