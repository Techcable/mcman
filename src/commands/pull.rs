use anyhow::{anyhow, bail, Context, Result};
use console::style;
use dialoguer::theme::ColorfulTheme;
use glob::glob;
use indexmap::IndexSet;
use indicatif::ProgressBar;
use itertools::Itertools;
use pathdiff::diff_paths;
use std::{fs, path::PathBuf, time::Duration};

use crate::app::App;

#[derive(clap::Args)]
pub struct Args {
    /// Files to pull (glob pattern)
    ///
    /// Exact duplicates are removed.
    #[arg(required = true)]
    files: Vec<String>,
    /// Treat all patterns as literals instead of as glob patterns.
    #[arg(long, short = 'F')]
    fixed_strings: bool,
}

pub fn run(app: &App, args: Args) -> Result<()> {
    let pb = app
        .multi_progress
        .add(ProgressBar::new_spinner())
        .with_message("Pulling files...");

    pb.enable_steady_tick(Duration::from_millis(250));

    let mut count = 0;
    let mut overwritten = 0;

    let entries = args
        .files
        .iter()
        .map(|file_spec| {
            if args.fixed_strings {
                Ok(vec![PathBuf::from(file_spec)])
            } else {
                glob(&*file_spec)?
                    .map(|x| x.map_err(anyhow::Error::new))
                    .collect::<Result<Vec<_>, _>>()
            }
        })
        .flatten_ok::<Vec<_>, _>()
        // using IndexSet implicitly removes exact duplicates, but not different names for the same file
        .collect::<Result<IndexSet<_>, _>>()?;
    for entry in entries {
        let absolute_entry = fs::canonicalize(&entry)?;

        let diff =
            diff_paths(&absolute_entry, fs::canonicalize(&app.server.path)?).ok_or_else(|| {
                anyhow!(
                    "Cannot diff paths {entry:?} and {server_path:?}",
                    server_path = &app.server.path
                )
            })?;

        if !diff.starts_with("server") {
            bail!("You aren't inside server/");
        }

        let mut destination = PathBuf::new();
        let mut iter = diff.components();
        iter.next().expect("Path to have atleast 1 component");
        destination.push(&app.server.path);
        destination.push("config");
        destination.extend(iter);

        fs::create_dir_all(destination.parent().unwrap()).context("Failed to create dirs")?;

        if destination.exists() {
            if app.confirm(&format!(
                "File '{}' already exists, overwrite?",
                destination.display()
            ))? {
                app.info(format!("Overwriting {}", destination.display()));
                overwritten += 1;
            } else {
                app.multi_progress.println(format!(
                    " {} {}",
                    style("Skipped overwriting").dim(),
                    destination.display(),
                ))?;
                continue;
            }
        }

        fs::copy(&entry, &destination)?;

        app.multi_progress.println(format!(
            " {} {} {} {}",
            ColorfulTheme::default().picked_item_prefix,
            style(&diff.to_string_lossy()).dim(),
            style("=>").bold(),
            style(
                diff_paths(
                    fs::canonicalize(&destination)?,
                    fs::canonicalize(&app.server.path)?
                )
                .unwrap_or_default()
                .to_string_lossy()
            )
            .dim()
        ))?;

        count += 1;
    }

    pb.finish_with_message(format!(
        " {} Pulled {} files to {}",
        ColorfulTheme::default().picked_item_prefix,
        count,
        style("config/").bold(),
    ));

    if overwritten != 0 {
        app.warn(format!("Overwrote {overwritten} files"));
    }

    Ok(())
}
