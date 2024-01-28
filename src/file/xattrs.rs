use std::{
	error::Error,
	io::{self, BufRead, BufReader, Write},
	os::unix::prelude::OsStrExt,
	process::{Child, ChildStdin},
};

use caps::{CapSet, Capability};
use log::info;

use crate::{cli::die, util::hex};

/// Writes a lossless textual representation of *all* extended attributes of `path` to `sink` by
/// requiring `CAP_SYS_ADMIN`. Explicitly not UTF-8 safe. Does not follow symlinks.
pub fn dump(
	xattr_helper: &mut Option<Helper>,
	path: &std::path::Path,
	m: &mut Box<dyn Write>,
) -> Result<(), Box<dyn Error>> {
	fn print_kv(sink: &mut Box<dyn Write>, key: Vec<u8>, value: Vec<u8>) -> io::Result<()> {
		write!(sink, "x k.")?;
		sink.write_all(&hex::tagged_rawhex::encode(true, &key))?;
		write!(sink, " v.")?;
		sink.write_all(&hex::tagged_rawhex::encode(false, &value))?;
		writeln!(sink)
	}

	// Note that we're explicitly not sorting by key here, to preserve some implementations-specific
	// information. For example, it seems key fetching order is consistent with key creation order,
	// although the spec makes no guarantees.
	match *xattr_helper {
		Some(Helper {
			ref mut process,
			ref mut stdin,
			ref mut stdout_lines,
		}) => {
			stdin.write_all(path.as_os_str().as_bytes())?;
			stdin.write_all(b"\0")?;

			for line in stdout_lines
				.map(|line_res| line_res.expect("reading from the helper should not fail"))
				.take_while(|line| line != "--")
			{
				let tokens: Vec<_> = line.split(' ').collect();

				print_kv(
					m,
					hex::decode(tokens[0].as_bytes()),
					hex::decode(tokens[1].as_bytes()),
				)?;
			}

			if let Ok(Some(status)) = process.try_wait() {
				die(
					exitcode::SOFTWARE,
					&format!(
						"Unexpected exit of `get-all-xattrs` with '{}', exiting.",
						status
					),
				);
			}
		}
		None => {
			// TODO: (C) can we do this with a context manager?
			log::trace!("raising CAP_SYS_ADMIN before getting xattrs");
			caps::raise(None, CapSet::Effective, Capability::CAP_SYS_ADMIN)?;

			for key in xattr::list(path).unwrap() {
				print_kv(
					m,
					key.as_bytes().to_owned(),
					// FIXME: deeper investigation into the double wrapping, swap the unwraps for
					// appropriate expects and error handling
					xattr::get(path, key).unwrap().unwrap(),
				)?;
			}

			log::trace!("dropping CAP_SYS_ADMIN after getting xattrs");
			caps::drop(None, CapSet::Effective, Capability::CAP_SYS_ADMIN)?;
		}
	};

	Ok(())
}

pub struct Helper {
	pub process: Child,
	pub stdin: ChildStdin,
	pub stdout_lines: std::io::Lines<BufReader<std::process::ChildStdout>>,
}

impl Helper {
	pub fn init_opt() -> Result<Option<Helper>, Box<dyn Error>> {
		use std::process::Stdio;

		use std::process::Command;

		Ok(
			if caps::has_cap(None, CapSet::Permitted, Capability::CAP_SYS_ADMIN)? {
				None
			} else {
				info!(
					"CAP_SYS_ADMIN capability not permitted, spawning `get-all-xattrs` to record \
                    `trusted.*` extended attributes."
				);

				// TODO: (S) look into cargo-parcel for distribution
				match Command::new("get-all-xattrs")
					.stdin(Stdio::piped())
					.stdout(Stdio::piped())
					.spawn()
				{
					Ok(mut process) => {
						let stdin = process
							.stdin
							.take()
							.expect("take should not fail for Stdio::piped()");

						let stdout = process
							.stdout
							.take()
							.expect("take should not fail for Stdio::piped()");

						let stdout_lines = BufReader::new(stdout).lines();

						Some(Helper {
							process,
							stdin,
							stdout_lines,
						})
					}
					Err(e) => die(
						exitcode::SOFTWARE,
						&format!(
							"Got {:?} while trying to spawn `get-all-xattrs`, exiting. Ensure that \
							you're not using '~' instead of the full path in your PATH variable.",
							e
						),
					),
				}
			},
		)
	}
}
