use anyhow::{anyhow, bail, Context, Result};
use console::style;
use dialoguer::theme::ColorfulTheme;
use glob::{glob, Pattern};
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
    /// Answer use to all prompts.
    #[arg(long)]
    yes: bool,
    /// Answer yes to all overwrite prompts.
    #[arg(long)]
    yes_overwrite: bool,
    /// Show what would be pulled without actually copying any files.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(app: &App, args: Args) -> Result<()> {
    let pb = app
        .multi_progress
        .add(ProgressBar::new_spinner())
        .with_message("Pulling files...");

    pb.enable_steady_tick(Duration::from_millis(250));

    let mut count = 0;
    let mut overwritten = 0;
    let mut ignored = 0;

    let ignore_patterns = app
        .server
        .options
        .pull_ignore
        .iter()
        .map(|p| Pattern::new(p).map_err(anyhow::Error::new))
        .collect::<Result<Vec<_>, _>>()?;

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

        let mut iter = diff.components();
        iter.next().expect("Path to have atleast 1 component");
        let relative: PathBuf = iter.collect();
        let relative_str = relative.to_string_lossy();

        if ignore_patterns.iter().any(|p| p.matches(&relative_str)) {
            app.multi_progress.println(format!(
                " {} {}",
                style("Ignored").dim(),
                style(&relative_str).dim()
            ))?;
            ignored += 1;
            continue;
        }

        let mut destination = PathBuf::new();
        destination.push(&app.server.path);
        destination.push("config");
        destination.push(&relative);

        if destination.exists() {
            if (args.yes_overwrite || args.yes)
                || (!args.dry_run
                    && app.confirm(&format!(
                        "File '{}' already exists, overwrite?",
                        destination.display()
                    ))?)
            {
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

        if !args.dry_run {
            fs::create_dir_all(destination.parent().unwrap()).context("Failed to create dirs")?;
            fs::copy(&entry, &destination)?;
        }

        app.multi_progress.println(format!(
            " {} {} {} {}",
            ColorfulTheme::default().picked_item_prefix,
            style(&diff.to_string_lossy()).dim(),
            style("=>").bold(),
            style(
                diff_paths(&destination, &app.server.path)
                    .unwrap_or_default()
                    .to_string_lossy()
            )
            .dim()
        ))?;

        count += 1;
    }

    pb.finish_with_message(format!(
        " {} {} {} files to {}",
        ColorfulTheme::default().picked_item_prefix,
        if args.dry_run { "Would pull" } else { "Pulled" },
        count,
        style("config/").bold(),
    ));

    if overwritten != 0 {
        app.warn(format!("Overwrote {overwritten} files"));
    }

    if ignored != 0 {
        app.info(format!("Ignored {ignored} files"));
    }

    Ok(())
}
