use std::{
	error::Error,
	ffi::OsString,
	fs::{self, File},
	io,
	os::unix::prelude::OsStringExt,
	path::{Path, PathBuf},
};

use log::debug;
use serde::Deserialize;

use crate::{
	repo::Repo,
	util::{dsv, ext::PathExt, nsv},
};

use super::snapshot::Snapshot;

pub const INCLUDES_NAME: &str = "include-paths.nsv";
pub const EXCLUDES_NAME: &str = "exclude-paths.nsv";

pub mod config_file {
	pub const NAME: &str = "config.toml";

	// Use macro to work around include_str not accepting string constants
	macro_rules! TEMPLATE_NAME_MACRO {
		() => {
			"site_config.toml"
		};
	}

	pub static DATA: &str = include_str!(concat!("../../templates/", TEMPLATE_NAME_MACRO!()));
}

#[derive(Deserialize)]
pub struct Config {
	pub exclude: ExcludeCfg,
}

#[derive(Deserialize)]
pub struct ExcludeCfg {
	pub cachedir_tag: bool,
	pub nodump: bool,
	pub all_eacces: bool,
}

#[derive(Debug)]
pub struct Site(pub PathBuf);

impl Site {
	pub const SNAPSHOTS_DIR_NAME: &'static str = "snaps";

	pub fn create<P: AsRef<Path>>(site_path: P) -> io::Result<()> {
		let site_path = site_path.as_ref();

		debug!("creating site dir {site_path:?}");
		fs::create_dir(site_path)?;

		debug!("creating include/exclude config files");
		File::create(site_path.join(INCLUDES_NAME))?;
		File::create(site_path.join(EXCLUDES_NAME))?;

		debug!("creating site config file");
		fs::write(site_path.join(config_file::NAME), config_file::DATA)
			.expect("unable to write to config file");

		debug!("creating snapshots dir");
		fs::create_dir(site_path.join(Self::SNAPSHOTS_DIR_NAME))
	}

	pub fn is_valid<P: AsRef<Path>>(dir: P) -> bool {
		dir.as_ref().is_dir()
			&& dir.as_ref().parent().is_some_and(|sites| {
				sites.file_name().is_some_and(|name| name == "sites")
					&& sites.parent().is_some_and(Repo::is_valid)
			})
	}

	pub fn from(de: std::fs::DirEntry) -> anyhow::Result<Site> {
		if !de.file_type()?.is_dir() {
			anyhow::bail!("Not a directory: {:?}", de.path())
		} else {
			Ok(Site(de.path()))
		}
	}

	pub fn repo(&self) -> Repo {
		Repo(
			self.0
				.parent()
				.expect("site dir should have a parent")
				.parent()
				.expect("sites_path should have a parent")
				.to_owned(),
		)
	}

	pub fn snaps_path(&self) -> PathBuf {
		self.0.join(Self::SNAPSHOTS_DIR_NAME)
	}

	pub fn get_config(&self) -> Result<Config, Box<dyn Error>> {
		Ok(toml::from_str(&std::fs::read_to_string(
			self.0.join(config_file::NAME),
		)?)?)
	}

	/// Returns tilde-expanded paths from the specified NSV file
	fn paths_from_file<P: AsRef<Path>>(file: P) -> std::io::Result<Vec<PathBuf>> {
		dsv::vec_from_file(file, nsv::SEP).map(|vectors| {
			vectors
				.into_iter()
				.map(|path_vec| PathBuf::from(OsString::from_vec(path_vec)).tilde_expand())
				.collect()
		})
	}

	pub fn get_included(&self) -> std::io::Result<Vec<PathBuf>> {
		Self::paths_from_file(self.0.join(INCLUDES_NAME))
	}

	pub fn get_excluded(&self) -> std::io::Result<Vec<PathBuf>> {
		Self::paths_from_file(self.0.join(EXCLUDES_NAME))
	}

	pub fn snapshots(&self) -> std::io::Result<Vec<io::Result<Snapshot>>> {
		self.0.join("snaps").read_dir().map(|iter| {
			iter.filter_map(|dentry_res| dentry_res.ok())
				.map(Snapshot::try_from)
				.collect()
		})
	}
}
