pub mod meta_file;
pub mod site;
pub mod snapshot;
pub mod tag_file;

use std::path::{Path, PathBuf};

use log::debug;

use self::site::Site;

pub struct Repo(pub PathBuf);

impl Repo {
	pub fn is_valid(dir: &Path) -> bool {
		tag_file::is_valid(dir.join(tag_file::NAME))
	}

	pub fn create(dir: &Path) -> std::io::Result<()> {
		tag_file::create_in(dir)?;

		debug!("creating sites subdirectory");
		std::fs::create_dir(dir.join("sites"))
	}

	pub fn sites(&self) -> std::io::Result<Vec<anyhow::Result<Site>>> {
		self.0.join("sites").read_dir().map(|iter| {
			iter.filter_map(|dentry_res| dentry_res.ok())
				.map(Site::from)
				.collect()
		})
	}
}
