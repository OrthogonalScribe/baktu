//! Contains the argument handling and main logic of the CLI

use std::env::current_dir;
use std::error::Error;
use std::ffi::{c_char, CString, OsString};
use std::fs::{self, read_dir, File};
use std::path::{Path, PathBuf};

use std::collections::{HashMap, HashSet};
use std::io::{self, stdout, ErrorKind, Read, Write};
use std::os::unix::prelude::OsStrExt;

use clap::{Args, Parser, Subcommand};
use exitcode::{ExitCode, CANTCREAT, DATAERR, IOERR, NOINPUT, SOFTWARE, USAGE};
use libc::faccessat;
use log::{debug, info, trace, warn, LevelFilter};
use nix::sys::stat::{mknod, Mode, SFlag};
use pathdiff::diff_paths;
use walkdir::DirEntry;

use crate::file::filekey::FileKey;
use crate::repo::site::Site;
use crate::repo::snapshot::Snapshot;
use crate::repo::{snapshot, Repo};
use crate::util::{hex, nsv};
use crate::{file, repo};

// Structure based on the recommendations in
// https://rust-cli-recommendations.sunshowers.io/handling-arguments.html

#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about,
    // TODO: (S) see if we can show this after subcommand long help too
    after_long_help = "\
    1. Logging: is controlled via the BAKTU_LOG and BAKTU_LOG_STYLES environment variables. Set \
    BAKTU_LOG to error, warn, info, debug or trace to adjust verbosity. For examples, see RUST_LOG \
    and RUST_LOG_STYLES mentions in https://docs.rs/env_logger/0.10.0/env_logger/\
    \n\
    \n\
    2. Capabilities: Extended attributes in the `trusted` namespace are only visible for processes \
    with the `CAP_SYS_ADMIN` capability. If the `baktu` executable is permitted to acquire that \
    capability (via `sudo setcap cap_sys_admin=p baktu`), it will acquire it for the duration of \
    reading each file's extended attributes. If baktu is not permitted to do so, it will use the \
    `get-all-xattrs` helper executable instead, ideally reducing the attack surface of the tool.\
    \n\
    \n\
    3. Excludes gotcha: explicit paths to be excluded are currently compared based on their \
    FileKey (stx_dev,stx_ino). This means that excluding one hard link will exclude all other \
    hard links. It is likely that baktu will switch to a lexical path canonicalization approach in \
    the future to (among other reasons) avoid this gotcha."
)]
pub struct Baktu {
	#[clap(flatten)]
	pub global_opts: GlobalOpts,

	#[clap(subcommand)]
	command: Command,
}

const PATH_HELP: &str = "\
	Path gotchas:\n\
	- Surround with single quotes to prevent shell expansion, e.g. to add a literal '~/docs', \
	keeping it user-relative.\n\
	- Symlinks with trailing slashes are interpreted as their targets, thus '~/ln_to_docs' \
	and '~/ln_to_docs/' will be considered different. See [POSIX.1-2017, Chapter 4.13] or the \
	discussion in \
	https://unix.stackexchange.com/questions/29769/trailing-slashes-on-symbolic-links-to-directories";

const NSV_HELP: &str = "\
	Null-separated value (NSV) files are files containing entries separated by ASCII NUL, i.e. \
	'\\0'. To inspect them, you can use something like `xargs -0n1 < file.nsv` or \
	`tr '\\0' '\\n' < file.nsv`, keeping in mind this will be misleading for entries containing \
	newlines.";

// TODO: (S) generate shell completions
#[derive(Debug, Subcommand)]
enum Command {
	/// Create a baktu repository in the current empty directory
	Init,

	// Not SiteAdd, so single-letter autocompletion doesn't clash with Snap
	/// Create a new site in the current baktu repository
	AddSite {
		/// Name of the site
		name: String,
	},

	/// Adds a path to a null-separated value file. See `--help` for notes on NSV files
	#[clap(after_long_help = NSV_HELP)]
	NsvAddTo {
		/// Path to the NSV file
		file: PathBuf,

		/// Entry to insert in the file. See `--help` for shell expansion and trailing slashes
		#[clap(long_help = PATH_HELP)]
		path: PathBuf,
	},

	/// Removes all occurrences of a path in a null-separated value file. See `--help` for notes on
	/// NSV files
	#[clap(after_long_help = NSV_HELP)]
	NsvRmFrom {
		/// Path to the NSV file
		file: PathBuf,

		/// Entry to remove from the file. See `--help` for shell expansion and trailing slashes
		#[clap(long_help = PATH_HELP)]
		path: PathBuf,
	},

	/// Create a new snapshot within the current site
	Snap(SnapArgs),
}

#[derive(Debug, Args)]
pub struct GlobalOpts {
	/// Verbosity level, can be specified multiple times, equivalent to BAKTU_LOG={info,debug,trace}
	#[arg(group="verbosity", long, short, global=true, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Quiet mode, equivalent to BAKTU_LOG=error
	#[arg(group = "verbosity", long, short, global = true)]
	pub quiet: bool,

	/// Silent mode, equivalent to BAKTU_LOG=off
	#[arg(group = "verbosity", long, short, global = true)]
	pub silent: bool,
}

#[derive(Debug, Args)]
struct SnapArgs {
	/// Do not error out on nonexistent exclude paths
	#[arg(long)]
	allow_nonexistent_exclude_paths: bool,

	/// Do nor report CACHEDIR.TAG files
	#[arg(long)]
	no_report_cachedir_tag: bool,

	/// Do not report unexcluded files with the nodump attribute
	#[arg(long)]
	no_report_nodump: bool,

	/// Required confirmation for exclude.all_eacces in the site config to work
	#[arg(long)]
	confirm_exclude_all_eacces: bool,

	/// Do not make any changes to the filesystem
	#[arg(short('n'), long)]
	dry_run: bool,
}

impl Baktu {
	pub fn exec(self) -> Result<(), Box<dyn Error>> {
		self.init_logging();

		info!("version {} starting up", env!("CARGO_PKG_VERSION"));
		info!("log level set to {}", log::max_level());

		// TODO: (S) refactor commands code to commands/<command>.rs
		use Command::*;
		match self.command {
			Init => Self::repo_init()?,
			AddSite { name } => Self::site_add(name)?,
			NsvAddTo { file, path } => nsv::append(&file, path.as_os_str().as_bytes())?,
			NsvRmFrom { file, path } => nsv::filter_not(file, path.as_os_str().as_bytes())?,
			Snap(args) => Self::snapshot(args)?,
		}

		info!("process exiting successfully");
		Ok(())
	}

	fn init_logging(&self) {
		let mut logging_builder = env_logger::Builder::new();

		// TODO: (S) does this handle BAKTU_LOG_STYLES too? Document it if it does, add it to the
		// roadmap otherwise
		logging_builder
			.filter_level(log::LevelFilter::Warn)
			.format_timestamp_nanos()
			.parse_env("BAKTU_LOG");

		// Not using https://crates.io/crates/clap-verbosity-flag as the documentation suggests
		// it may not work with the [default -> env -> cli-args] override path
		let override_log_level = if self.global_opts.silent {
			Some(log::LevelFilter::Off)
		} else if self.global_opts.quiet {
			Some(log::LevelFilter::Error)
		} else {
			match self.global_opts.verbose {
				0 => None,
				1 => Some(log::LevelFilter::Info),
				2 => Some(log::LevelFilter::Debug),
				_ => Some(log::LevelFilter::Trace),
			}
		};

		if let Some(new_level) = override_log_level {
			logging_builder.filter_level(new_level);
		}

		logging_builder.init();
	}

	fn repo_init() -> io::Result<()> {
		let cwd = current_dir()?;

		if let Some(_dentry) = read_dir(&cwd)?.next() {
			die(USAGE, "current directory is not empty, exiting")
		}

		Repo::create(&cwd)?;

		info!("init subcommand done");

		Ok(())
	}

	fn site_add(name: String) -> io::Result<()> {
		let sites_path = repo_root_or_die()?.join("sites");

		if !sites_path.exists() {
			die(DATAERR, "repo corrupt: sites directory does not exist")
		}

		let site_path = sites_path.join(name);
		if site_path.exists() {
			die(USAGE, "a site with that name already exists")
		}

		Site::create(&site_path)?;

		info!("add-site subcommand done");

		Ok(())
	}

	fn snapshot(cfg: SnapArgs) -> Result<(), Box<dyn Error>> {
		let site = repo_site_or_die()?;

		let includes = {
			let result = site.get_included()?;
			if result.is_empty() {
				die(
					DATAERR,
					&format!(
						"no paths have been included, run `baktu nsv-add-to {} <PATH>` first",
						repo::site::INCLUDES_NAME
					),
				)
			}

			let nonexistent: Vec<_> = result.iter().filter(|path| !path.exists()).collect();
			if !nonexistent.is_empty() {
				// Print them all out, so we don't have to do an edit-rerun loop in case of multiple
				// nonexistent ones
				for p in nonexistent {
					log::error!("included path {p:?} doesn't exist")
				}
				die(DATAERR, "found nonexistent included paths")
			} else {
				result
			}
		};

		// TODO: (S) decide how to handle exclude paths outside of srcRoot, e.g. error out + add an
		// allow flag
		let excludes: HashSet<FileKey> = {
			let seq = site.get_excluded()?;
			let nonexistent: Vec<_> = seq.iter().filter(|path| !path.exists()).collect();
			if !nonexistent.is_empty() {
				// Print them all out, so we don't have to do an edit-rerun loop in case of multiple
				// nonexistent ones
				for p in nonexistent {
					log::error!("excluded path {p:?} doesn't exist")
				}
				die(DATAERR, "found nonexistent excluded paths")
			} else {
				HashSet::from_iter(
					seq.into_iter()
						.map(|path| FileKey::from_path(&path).expect("unable to create FileKey")),
				)
			}
		};

		let site_conf = site.get_config()?;

		// Excluded path counts from the is_included lambda, and from the rest of the main loop,
		// respectively
		let mut excluded_cnt_is_included = 0u64;
		let mut excluded_cnt_loop = 0u64;

		let mut is_included = |dir_entry: &DirEntry| {
			trace!("testing is_included({:?})", &dir_entry);
			let mut exclude = |reason| -> bool {
				info!("excluding {:?} due to {}", dir_entry.path(), reason);
				excluded_cnt_is_included += 1;
				false
			};

			let mut die_or_log_exclude_all_eacces = |denied_action| {
				if site_conf.exclude.all_eacces && cfg.confirm_exclude_all_eacces {
					exclude(
						repo::site::config_file::NAME.to_owned()
							+ &format!("/exclude.all_eacces ({denied_action})"),
					);
				} else {
					die(
						NOINPUT,
						&format!(
							// TODO: (C) look into capabilities or other security
							//	mechanisms as a more fine-grained way to allow baktu
							//	access to [path]
							// TODO: (S) DRY the confirm flag in all locations
							"Permission denied during {denied_action} for {:?}, exiting. You can \
								either \
								1) exclude the path explicitly and re-run, \
								2) re-run `baktu` with sudo or equivalent, or \
								3) set `exclude.all_eacces` in the site `{}` and \
									re-run with `--confirm-exclude-all-eacces`",
							&dir_entry.path(),
							repo::site::config_file::NAME
						),
					)
				}
			};

			let stx = match file::statx::get(dir_entry.path()) {
				Ok(res) => res,
				Err(e) if e.kind() == ErrorKind::PermissionDenied => {
					die_or_log_exclude_all_eacces("statx");
					return false;
				}
				Err(e) => die(
					SOFTWARE,
					&format!(
						"statx({:?}): unexpected error {:?}, exiting",
						&dir_entry.path(),
						e,
					),
				),
			};

			// TODO: (S) switch to lexical canonicalization instead:
			//   1. Fixes hard link aliasing issue
			//   2. May save us an extra system call in some situations
			let fk = FileKey::from_statx(&stx);

			// TODO: (C) consider flattening the decision tree to improve readability, if we can do
			// so without increasing the risk of bugs too much
			if excludes.contains(&fk) {
				exclude(repo::site::EXCLUDES_NAME.to_owned())
			} else {
				// obeying clippy's lint here would result in less obvious code structure
				#[allow(clippy::collapsible_else_if)]
				if site_conf.exclude.cachedir_tag
					&& dir_entry.file_type().is_dir()
					&& dir_entry.path().join("CACHEDIR.TAG").exists()
					&& file::is_valid_cachedir_tag(dir_entry.path().join("CACHEDIR.TAG").as_path())
				{
					exclude(repo::site::config_file::NAME.to_owned() + "/exclude.cachedir_tag")
				} else {
					if site_conf.exclude.nodump
						&& stx.stx_attributes_mask & libc::STATX_ATTR_NODUMP as u64 != 0
						&& stx.stx_attributes & libc::STATX_ATTR_NODUMP as u64 != 0
					{
						exclude(repo::site::config_file::NAME.to_owned() + "/exclude.nodump")
					} else {
						fn readable(p: &Path) -> bool {
							/* TODO: (S) consider switching to nix, as AT_EACCESS is now supported
								- issue: https://github.com/nix-rust/nix/pull/1995
								- in since 0.27.0, see
									https://github.com/nix-rust/nix/blob/master/CHANGELOG.md
							// recommended in https://github.com/nix-rust/nix/issues/1340
							const dirfd: libc::c_int = libc::AT_FDCWD;
							nix::unistd::faccessat(
								Some(dirfd),
								p,
								nix::unistd::AccessFlags::R_OK,
								AT_EACCESS | nix::fcntl::AtFlags::AT_SYMLINK_NOFOLLOW).is_ok()
							*/

							// from https://docs.rs/faccess/0.2.4/src/faccess/lib.rs.html#92
							// modified with AT_SYMLINK_NOFOLLOW
							let path =
								CString::new(p.as_os_str().as_bytes()).expect("p can't contain 0");

							unsafe {
								faccessat(
									libc::AT_FDCWD,
									path.as_ptr() as *const c_char,
									libc::R_OK,
									libc::AT_EACCESS | libc::AT_SYMLINK_NOFOLLOW,
								) == 0
							}
						}

						if !readable(dir_entry.path()) {
							die_or_log_exclude_all_eacces("faccessat(READ)");
							false
						} else {
							true
						}
					}
				}
			}
		};

		let mut xattr_helper = file::xattrs::Helper::init_opt()?;

		// FIXME: remove the hardcoding once we can create more than the initial snapshot
		// TODO: (S) refactor this into a Site impl fn
		const ZEROTH_SNAP_NAME: &str = "0";
		let snap_path = site.snaps_path().join(ZEROTH_SNAP_NAME);

		// TODO: (S) abstract over run dryness, so we end up with a single `if cfg.dry_run`
		if cfg.dry_run {
			info!("(fake) creating snapshot dir {snap_path:?}");
		} else {
			info!("creating snapshot dir {snap_path:?}");
			fs::create_dir(&snap_path)?;
		}

		// TODO: (S) strongly consider converting to Snapshot type earlier
		// TODO: (S) smaller-scope: get rid of unnecessary clone
		let snap_data_path = Snapshot::from_dryable(snap_path.clone(), cfg.dry_run)?.data_dir();
		if cfg.dry_run {
			info!("(fake) creating snapshot data dir {snap_data_path:?}");
		} else {
			info!("creating snapshot data dir {snap_data_path:?}");
			fs::create_dir(&snap_data_path)?;
		}

		let meta_name: OsString = {
			// FIXME: should find a unique META_NAME given the current input file name set, so we
			// never get collisions. Adjust accordingly if we start supporting things that might
			// break the invariants, for example incremental snapshotting.
			".baktu.meta.brj".into()
		};
		let meta_name_fpath = snap_path.join(snapshot::META_NAME_FNAME);
		if cfg.dry_run {
			info!("(fake) writing metadata file name to {meta_name_fpath:?}");
		} else {
			info!("writing metadata file name to {meta_name_fpath:?}");
			std::fs::OpenOptions::new()
				.create_new(true)
				.write(true)
				.open(meta_name_fpath)?
				.write_all(meta_name.as_bytes())?;
		}

		let mut processed_cnt = 0u64;

		// TODO: (S) consider the trade-offs of doing a lazy initialization here, especially if we
		// switch to duplicate detection mechanisms that require slower initialization. We'll need
		// this "only" when we have to process regular files, so in the spirit of KISS we can
		// consider that to always be the case, with rare and unusual exceptions.

		// Map from file hash to set of destination paths of files with that hash. Excludes files
		// too small to be deduplicated.
		let mut hash2paths: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();

		// TODO: (S) dedup code below, figure out what I don't understand to make it work with the
		//	borrow checker.
		// let mut add_hash_path = |h: Hash, p: PathBuf| {
		// };

		// TODO: (C) consider if using a segment trie for path storage would make
		//   sense - trading space (RAM, in this case) for time - given that we'll
		//   be dealing with something on the order of 10^5 entries at a minimum.

		// TODO: (S) make projections for when we'll need to switch to a more
		//   appropriate key-value storage approach, do preliminary research on
		//   possible alternatives (e.g. sqlite) and their trade-offs.
		for site_res in site.repo().sites()? {
			for snap_res in site_res.expect("site error").snapshots()? {
				for meta_res in snap_res.expect("snap error").meta_files()? {
					let meta = meta_res.expect("meta_file error");
					for record in meta.records()? {
						if let Some((h, p)) = record.get_hash_path_opt(&meta.0)? {
							// add_hash_path(h, p);
							// TODO: (C) consider keeping a histogram of bucket sizes while
							// figuring out how much of the file to use for fake_b3sum
							match hash2paths.get_mut(&h) {
								Some(paths) => {
									warn!("hash collision with file {p:?}");
									paths.push(p)
								}
								None => {
									hash2paths.insert(h, vec![p]);
								}
							}
						}
					}
				}
			}
		}

		// FIXME: handle roots with the same basename appropriately
		for include_root in includes {
			info!("processing include_root {include_root:?}");
			let include_root = {
				let resolved = fs::canonicalize(&include_root).expect("could not canonicalize");
				if resolved != include_root {
					info!("\twhich resolves to {resolved:?}");
				}
				resolved
			};

			let dst_inc_root_path = snap_data_path.join(
				include_root
					.file_name()
					.expect("canonicalized path can not contain trailing .."),
			);

			// TODO: (C) improve performance by DIYing the walking, since for each dentry:
			//   - WalkDir has an fd
			//   - WalkDir does a statx
			//   - we do a statx to generate the FileKey
			//   - we later do a statx again (up to and including DIOALIGN)
			//   - we'll need an fd for FS_IOC_GETFLAGS
			for entry in walkdir::WalkDir::new(&include_root)
				.sort_by_file_name()
				.into_iter()
				.filter_entry(&mut is_included)
			{
				// FIXME: handle permission errors. Error out and suggest to a) fix permissions,
				// b) exclude, or c) re-run with appropriate UID/GID/permissions
				let entry = entry?;

				let path = entry.path();
				debug!("processing path {path:?}");

				// This should stay even after we switch to finding a meta_name that doesn't occur
				// in the source file set, so we can fail early and loudly in TOCTOU situations,
				// instead of risking data corruption
				if entry.file_type().is_dir() && entry.path().join(&meta_name).exists() {
					die(
						DATAERR,
						&format!(
							"source dir {:?} already contains file named {meta_name:?}",
							entry.path()
						),
					);
				}

				if !cfg.no_report_cachedir_tag
					&& entry.file_type().is_file()
					&& entry.file_name() == "CACHEDIR.TAG"
				{
					if file::is_valid_cachedir_tag(entry.path()) {
						warn!(
							"Found valid and unexcluded CACHEDIR.TAG at {:?}. Rerun with \
							--no-report-cachedir-tag or enable 'exclude.cachedir_tag' in the site \
							'{}' to hide this warning.",
							entry.path(),
							repo::site::config_file::NAME
						);
					} else {
						warn!(
							"Found invalid CACHEDIR.TAG at {:?}. Rerun with \
							--no-report-cachedir-tag to hide this warning.",
							entry.path()
						);
					}
				}

				let stx = file::statx::get(path).expect(
					"statx should not fail as it has already \
					been invoked on the path in is_included()",
				);

				if !cfg.no_report_nodump
					&& stx.stx_attributes_mask & libc::STATX_ATTR_NODUMP as u64 != 0
					&& stx.stx_attributes & libc::STATX_ATTR_NODUMP as u64 != 0
				{
					warn!(
						"{:?} is marked as nodump, but not excluded. Rerun with --no-report-nodump \
						or enable 'exclude.nodump' in the site '{}' to hide this warning.",
						entry.path(),
						repo::site::config_file::NAME
					);
				}

				let root_rel_path = path.strip_prefix(&include_root)?;
				let dst_path = if root_rel_path == Path::new("") {
					// include root is a file, not a directory
					dst_inc_root_path.clone()
				} else {
					dst_inc_root_path.join(root_rel_path)
				};

				let hash = if entry.file_type().is_file() {
					Some(file::b3sum(path)?)
				} else {
					None
				};

				let repo_find_dup =
					|hash: blake3::Hash, path: &Path| -> io::Result<Option<PathBuf>> {
						// Short-circuiting byte-by-byte comparison.
						// Of course the usual considerations apply:
						// - read files in chunks [f9]
						// - compare at least usize bytes at a time [f10]
						// - SIMD if easy enough, might help or not
						//
						// cmp perf with 4K in case we get no gains from going down to a "sector"
						// size, see [f9]
						//
						// semi-related: https://lib.rs/crates/dupe-krill from [f11] describes an
						// interesting approach to dedup via LazilyHashing<File> -> Vec<Path>

						// [f9]  https://users.rust-lang.org/t/efficient-way-of-checking-if-two-files-have-the-same-content/74735/9
						// [f10] https://users.rust-lang.org/t/efficient-way-of-checking-if-two-files-have-the-same-content/74735/10
						// [f11] https://users.rust-lang.org/t/efficient-way-of-checking-if-two-files-have-the-same-content/74735/11
						fn file_cmp(p1: &Path, p2: &Path) -> io::Result<bool> {
							let mut f1 = File::open(p1)?;
							let mut f2 = File::open(p2)?;

							if f1.metadata()?.len() != f2.metadata()?.len() {
								return Ok(false);
							}

							const BUF_SIZE: usize = 64 * 1024;
							let b1 = &mut [0; BUF_SIZE];
							let b2 = &mut [0; BUF_SIZE];

							loop {
								let f1_read_len = f1.read(b1)?;
								let f2_read_len = f2.read(b2)?;

								if f1_read_len != f2_read_len {
									die(IOERR,
										&format!("mismatched read lengths on equally sized files {p1:?} and {p2:?}"));
								}

								// Second part is redundant, but keep it in for robustness
								if f1_read_len == 0 && f2_read_len == 0 {
									return Ok(true);
								}

								if b1[0..f1_read_len] != b1[0..f2_read_len] {
									return Ok(false);
								}
							}
						}

						// might need to bump fake_b3sum to 512 or more bytes if too many false
						// positives. See "histogram" TODO above
						match hash2paths.get(&hash) {
							None => Ok(None),
							Some(paths) => {
								for candidate in paths {
									if file_cmp(path, candidate)? {
										return Ok(Some(candidate.to_path_buf()));
									}
								}
								Ok(None)
							}
						}
					};

				// Only true if we deduplicate the file. False even if there's other files with the
				// same content, but we choose to not deduplicate (e.g. due to size too small)
				let mut is_deduplicated = false;

				debug!("creating at destination {dst_path:?}");
				// Note that initially we're only focusing on recreating the non-meta state of the
				// file, as all meta-information should be recorded in the meta dump afterwards.
				// However recreating more of the meta state (permissions, etc) is a Could, or
				// ideally even a Should task for later, as this would provide more of the source
				// state at later stages of the repo's graceful degradation.
				assert!(stx.stx_mask & libc::STATX_TYPE != 0);
				match stx.stx_mode as u32 & libc::S_IFMT {
					libc::S_IFDIR => {
						if cfg.dry_run {
							info!("(fake) mkdir {dst_path:?}")
						} else {
							fs::create_dir(&dst_path)?
						}
					}
					libc::S_IFREG => {
						// We ought to need at least a byte to create a meaningful symlink when
						// deduplicating, thus a minimum sensible threshold would be larger.
						const DEDUP_MIN_FSIZE: u64 = 2;
						if entry.metadata()?.len() < DEDUP_MIN_FSIZE {
							debug!("skipping deduplication of file smaller than {DEDUP_MIN_FSIZE} bytes");
							if cfg.dry_run {
								info!("(fake) cp {path:?} {dst_path:?}");
							} else {
								fs::copy(path, &dst_path)?;
							}
						} else {
							// TODO: (C) consider encoding file type + hash as a sum type
							// [later edit] possibly moot, as we don't always need the hash here
							let hash = hash.expect("hash not Some while REG");
							match repo_find_dup(hash, path)? {
								Some(preexisting_path) => {
									is_deduplicated = true;
									if cfg.dry_run {
										info!(
											"(fake) dedup src={path:?} \
											dst={dst_path:?} preexisting={preexisting_path:?}"
										);
									} else {
										// Pros/cons of using hard links:
										//	- introduces limits - 65K on ext4, according to
										//		https://unix.stackexchange.com/questions/5629/is-there-a-limit-of-hardlinks-for-one-file
										//	- removes the possibility to optimize duplicate
										//		detection in repo clients by checking
										//		meta.is_deduplicated only for symlinks
										//	- introduces hard links into the repo, which introduces
										//		additional concerns for operating on it with tar,
										//		rsync, etc.
										//	± duplicate representation at the data level does not
										//		depend on which one is encountered first. Meta level
										//		is still affected, which might need to be taken into
										//		account
										//	+ presumably saves a few bytes in some cases, as we can
										//		just use a dentry, not needing space for the
										//		relative path.

										// create relative symlink to the preexisting path
										// TODO: (M) test that moving a baktu repo doesn't break
										// these
										std::os::unix::fs::symlink(
											diff_paths(
												preexisting_path,
												dst_path.parent().expect("dedup dest has parent"),
											)
											.expect("should work for 2 absolute paths"),
											&dst_path,
										)?;
									}
								}
								None => {
									if cfg.dry_run {
										info!("(fake) cp {path:?} {dst_path:?}");
									} else {
										fs::copy(path, &dst_path)?;
									}

									// We use the source path when we're "creating" a dry-run
									// snapshot, so there's something to compare for subsequent
									// dedup byte-by-byte checks
									let backing_path = if cfg.dry_run {
										path.to_path_buf()
									} else {
										dst_path.clone()
									};

									trace!("new data, adding to map: {hash:?} {backing_path:?}");
									// add_hash_path(hash, dst_path);

									// TODO: (C) consider keeping a histogram of bucket sizes while
									// figuring out how much of the file to use for fake_b3sum, *if*
									// we switch back to fake_b3sum
									match hash2paths.get_mut(&hash) {
										Some(paths) => {
											warn!("hash collision with file {backing_path:?}");
											paths.push(backing_path)
										}
										None => {
											hash2paths.insert(hash, vec![backing_path]);
										}
									}
								}
							}
						}
					}
					libc::S_IFLNK => {
						// TODO: (M) ensure this preserves the symlink as is
						std::os::unix::fs::symlink(fs::read_link(path)?, &dst_path)?
					}
					libc::S_IFBLK | libc::S_IFCHR | libc::S_IFIFO | libc::S_IFSOCK => {
						assert!(stx.stx_mask & libc::STATX_MODE != 0);
						assert!(stx.stx_mask & libc::STATX_TYPE != 0);

						// S_IFMT bit twiddling from
						// https://man7.org/linux/man-pages/man2/statx.2.html

						// dev ignored if not CHR/BLK, according to
						// https://man7.org/linux/man-pages/man2/mknod.2.html
						match mknod(
							&dst_path,
							SFlag::from_bits(stx.stx_mode as u32 & libc::S_IFMT)
								.expect("bits to kind"),
							Mode::from_bits(stx.stx_mode as u32 & !libc::S_IFMT)
								.expect("bits to perms"),
							libc::makedev(stx.stx_rdev_major, stx.stx_rdev_minor),
						) {
							Ok(_) => (),
							Err(nix::errno::Errno::EPERM) => {
								// TODO: (M) DRY the exclude/exit logic here
								if site_conf.exclude.all_eacces && cfg.confirm_exclude_all_eacces {
									info!(
										"excluding {:?} due to {} (mknod)",
										entry.path(),
										repo::site::config_file::NAME.to_owned()
											+ "/exclude.all_eacces"
									);
									excluded_cnt_loop += 1;
									continue;
								} else {
									die(
										CANTCREAT,
										&format!(
											// TODO: (C) look into capabilities or other security
											//	mechanisms as a more fine-grained way to allow baktu
											//	access to [path]
											// TODO: (S) DRY the confirm flag in all locations
											"Permission denied during mknod for {:?}, exiting. You \
												can either \
												1) exclude the path explicitly and re-run, \
												2) re-run `baktu` with sudo or equivalent, or \
												3) set `exclude.all_eacces` in the site `{}` and \
													re-run with `--confirm-exclude-all-eacces`",
											&entry.path(),
											repo::site::config_file::NAME
										),
									)
								}
							}
							Err(e) => die(
								SOFTWARE,
								&format!(
									"mknod({:?}): unexpected error {:?}, exiting",
									&entry.path(),
									e,
								),
							),
						};
					}
					ft => die(DATAERR, &format!("unknown file type {ft}")),
				}

				dump_meta(
					&mut xattr_helper,
					get_meta_sink(cfg.dry_run, &dst_path, &meta_name),
					path,
					stx,
					hash,
					is_deduplicated,
				)?;

				processed_cnt += 1;
			}
		}

		info!(
			"{} files excluded (not counting children), {} files processed",
			excluded_cnt_is_included + excluded_cnt_loop,
			processed_cnt
		);

		// xattr_helper: no need for explicit cleanup, as it will automatically have its stdin
		// closed, and will exit normally

		info!("snapshot subcommand done");

		Ok(())
	}
}

fn get_meta_sink(dry_run: bool, dst_path: &Path, meta_name: &OsString) -> Box<dyn Write> {
	if dry_run {
		if log::max_level() >= LevelFilter::Debug {
			debug!("dumping meta to stdout");
			Box::new(stdout())
		} else {
			Box::new(
				std::fs::OpenOptions::new()
					.append(true)
					.open("/dev/null")
					.expect("opening /dev/null should not fail"),
			)
		}
	} else {
		// Can be optimized to be calculated once per entering a directory, at the trade-off
		// of keeping mutable context between loop iterations due to the current flattened
		// mode of walking the tree
		let meta_path = dst_path
			.parent()
			.expect("parent must exist given definition of dst_path")
			.join(meta_name);
		debug!("dumping metadata to {meta_path:?}");
		Box::new(
			std::fs::OpenOptions::new()
				.append(true)
				.create(true)
				.open(meta_path)
				.expect("should not fail if the FIXME above is implemented"),
			// [edit] reference above no longer clear. Possibly the non-collision meta filename
		)
	}
}

fn dump_meta(
	xattr_helper: &mut Option<file::xattrs::Helper>,
	mut sink: Box<dyn Write>,
	path: &std::path::Path,
	stx: libc::statx,
	hash: Option<blake3::Hash>,
	is_deduplicated: bool,
) -> Result<(), Box<dyn Error>> {
	// Write this first, so we don't waste time while building the hash->path map during dedup
	if is_deduplicated {
		sink.write_all(repo::meta_file::line::IS_DEDUPLICATED)?;
		writeln!(sink)?;
	}

	// TODO: (M) switch to space-separated key-raw/hex pairs for all the other keys
	sink.write_all(repo::meta_file::line::PFX_NAME)?;
	write!(sink, " ")?;
	sink.write_all(&hex::tagged_rawhex::encode(
		false,
		path.file_name()
			.expect("has last component, not ending in ..")
			.as_bytes(),
	))?;
	writeln!(sink)?;

	if let Some(h) = hash {
		sink.write_all(repo::meta_file::line::PFX_HASH)?;
		writeln!(sink, " {}", h.to_hex())?;
	}

	file::statx::dump(stx, &mut sink)?;

	// lsattr
	assert!(stx.stx_mask & libc::STATX_TYPE != 0);
	use libc::{S_IFBLK, S_IFCHR, S_IFIFO, S_IFLNK, S_IFSOCK};
	match stx.stx_mode as u32 & libc::S_IFMT {
		// FS_IOC_GETFLAGS not supported on char/block devices, see ioctl supported only
		// for dirs and regular files, see also
		// https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=152029
		S_IFCHR | S_IFBLK => (),
		// Symlinks:
		// - https://lore.kernel.org/linux-xfs/20171101235007.GF22894@wotan.suse.de/T/
		// - getting a fd usable by the ioctl proves to be difficult
		// Sockets fail too
		S_IFSOCK | S_IFLNK => (),
		// Causes hang on ~/.steam/steam.pipe
		S_IFIFO => (),
		// At the moment we emulate lsattr(1), and only handle regular files and
		// directories
		_otherwise => file::ioctl_getflags::dump(path, &mut sink)?,
	}

	file::xattrs::dump(xattr_helper, path, &mut sink)?;

	writeln!(sink, "--")?;

	Ok(())
}

fn repo_root_or_die() -> io::Result<PathBuf> {
	let cwd = current_dir()?;
	let Some(root) = cwd.ancestors().find(|p| Repo::is_valid(p)) else {
		die(
			USAGE,
			"not in a baktu repository, exiting. `cd` into an existing one, or create one via \
			`baktu init <repo_name>`",
		)
	};
	Ok(root.to_path_buf())
}

fn repo_site_or_die() -> io::Result<Site> {
	let cwd = current_dir()?;
	let Some(site_path) = cwd.ancestors().find(|p| Site::is_valid(p)) else {
		die(
		USAGE,
			"not in a baktu repository site, exiting. `cd` into an existing one, or create one via \
			`baktu add-site <site_name>`"
		)
	};
	Ok(Site(site_path.to_owned()))
}

// TODO: (S) look into reorganizing code to only invoke die() in the CLI code
// TODO: (S) find if we can avoid having to manually do &format!() for [msg]
pub fn die(code: ExitCode, msg: &str) -> ! {
	log::error!("{}", msg);
	std::process::exit(code)
}
