use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    time::Duration,
};

use anyhow::Result;
use indicatif::{FormattedDuration, ProgressBar, ProgressIterator, ProgressStyle};
use tokio::fs;

use crate::app::AddonType;

use super::BuildContext;

impl BuildContext<'_> {
    pub async fn download_addons(&mut self, addon_type: AddonType) -> Result<()> {
        let server_list = self.app.get_addons(addon_type);
        let addons = match addon_type {
            AddonType::Plugin => &self.lockfile.plugins,
            AddonType::Mod => &self.lockfile.mods,
        };

        let existing_files = addons.iter().fold(
            HashSet::with_capacity(addons.len()),
            |mut hash_set, (_, res)| {
                hash_set.insert(res.filename.clone());
                hash_set
            },
        );

        // pinned addons unchanged since the last build can reuse their lockfile
        // entry instead of re-resolving against the source's API
        let locked: HashMap<_, _> = addons.iter().map(|(dl, res)| (dl, res)).collect();

        if server_list.is_empty() && existing_files.is_empty() {
            self.warn_unrecognized_files(addon_type, &HashSet::new())
                .await?;
            return Ok(());
        }

        self.app.print_job(&format!(
            "Processing {} {addon_type}{}...{}",
            server_list.len(),
            if server_list.len() == 1 { "" } else { "s" },
            if server_list.len() < 200 {
                ""
            } else {
                " may god help you"
            },
        ));

        self.app.ci(&format!("::group::Processing {addon_type}s"));

        let mut files_list = HashSet::with_capacity(server_list.len());

        let pb = ProgressBar::new(server_list.len() as u64)
            .with_style(ProgressStyle::with_template(
                "{msg} [{wide_bar:.cyan/blue}] {pos}/{len}",
            )?)
            .with_message(format!("Processing {addon_type}s"));

        let pb = self.app.multi_progress.add(pb);

        for addon in server_list.iter().progress_with(pb.clone()) {
            let cached = if !self.force && addon.is_pinned() {
                locked
                    .get(addon)
                    .filter(|res| !res.hashes.is_empty())
                    .map(|res| (*res).clone())
            } else {
                None
            };

            let (_path, resolved) = self
                .downloadable_with_cache(addon, cached, addon_type.folder(), Some(&pb))
                .await?;

            files_list.insert(resolved.filename.clone());

            match addon_type {
                AddonType::Plugin => &mut self.new_lockfile.plugins,
                AddonType::Mod => &mut self.new_lockfile.mods,
            }
            .push((addon.clone(), resolved));
        }

        pb.set_style(ProgressStyle::with_template(
            "{spinner:.blue} {prefix:.yellow} {msg}",
        )?);
        pb.set_prefix("Deleting");
        pb.enable_steady_tick(Duration::from_micros(250));

        for removed_file in existing_files.difference(&files_list) {
            pb.set_message(removed_file.clone());
            match fs::remove_file(self.output_dir.join(addon_type.folder()).join(removed_file))
                .await
            {
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    self.app.warn(
                        "File scheduled to be deleted did not exist, possibly deleted externally",
                    );
                    Ok(())
                }
                o => o,
            }?;
        }

        pb.finish_and_clear();

        self.warn_unrecognized_files(addon_type, &files_list)
            .await?;

        if files_list.len() >= 10 {
            self.app.success(format!(
                "Processed {} {addon_type}{} in {}",
                files_list.len(),
                if files_list.len() == 1 { "" } else { "s" },
                FormattedDuration(pb.elapsed())
            ));
        }

        self.app.ci("::endgroup::");

        Ok(())
    }

    /// Warns about `.jar` files directly inside the addon folder that don't
    /// correspond to any declaration in `server.toml`. Doesn't touch them -
    /// unlike the build-time pruning (`build_prune_include`), this is purely
    /// informational, since files here (private/proprietary jars, manual test
    /// builds, etc.) are often placed there intentionally.
    async fn warn_unrecognized_files(
        &self,
        addon_type: AddonType,
        files_list: &HashSet<String>,
    ) -> Result<()> {
        let folder = self.output_dir.join(addon_type.folder());

        let mut entries = match fs::read_dir(&folder).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }

            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jar") {
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();

            if files_list.contains(&name) {
                continue;
            }

            self.app.warn(format!(
                "Unrecognized file in {}/: '{name}' - not declared in server.toml, leaving it as is",
                addon_type.folder(),
            ));
        }

        Ok(())
    }
}
