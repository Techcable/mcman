use std::borrow::Cow;

use anyhow::Result;
use indexmap::IndexMap;
use indicatif::{ProgressBar, ProgressStyle};

use crate::{
    app::{AddonType, App},
    model::{Downloadable, ServerType},
    util::md::MarkdownTable,
};

#[derive(clap::Args)]
pub struct Args {
    /// Also consider non-release Hangar channels (eg. Beta, Alpha, Snapshot) when
    /// looking for updates. By default only the Release channel is considered.
    #[arg(long)]
    all_channels: bool,
}

enum CheckResult {
    UpToDate,
    Outdated { current: String, latest: String },
}

/// Whether `check_update` supports this downloadable: it must come from Modrinth,
/// Hangar or Spigot, and be pinned to an explicit version rather than a floating
/// `"latest"` (which is already up to date by definition on every build).
fn is_checkable(dl: &Downloadable) -> bool {
    match dl {
        Downloadable::Modrinth { version, .. }
        | Downloadable::Hangar { version, .. }
        | Downloadable::Spigot { version, .. } => version != "latest",
        _ => false,
    }
}

async fn check_update(app: &App, dl: &Downloadable, all_channels: bool) -> Result<CheckResult> {
    Ok(match dl {
        Downloadable::Modrinth { id, version } => {
            let current = app.modrinth().fetch_version(id, version).await?;
            let latest = app.modrinth().fetch_version(id, "latest").await?;

            if current.id == latest.id {
                CheckResult::UpToDate
            } else {
                CheckResult::Outdated {
                    current: current.version_number,
                    latest: latest.version_number,
                }
            }
        }
        Downloadable::Hangar { id, version } => {
            let current = app.hangar().fetch_hangar_version(id, version).await?;
            let latest = app.hangar().fetch_newest_version(id, all_channels).await?;

            if current.name == latest.name {
                CheckResult::UpToDate
            } else {
                CheckResult::Outdated {
                    current: current.name,
                    latest: latest.name,
                }
            }
        }
        Downloadable::Spigot { id, version } => {
            let current = app.spigot().fetch_version(id, version).await?;
            let latest = app.spigot().fetch_version(id, "latest").await?;

            if current.id == latest.id {
                CheckResult::UpToDate
            } else {
                CheckResult::Outdated {
                    current: current.name,
                    latest: latest.name,
                }
            }
        }
        _ => unreachable!("check_update() called on a non-checkable downloadable"),
    })
}

/// Reports plugins, mods and the server jar (when pinned to Modrinth, Hangar or
/// Spigot) that have a newer version available upstream. Read-only: it never
/// edits server.toml or the lockfile, it only prints what could be updated.
pub async fn run(app: App, args: Args) -> Result<()> {
    let mut targets: Vec<(&'static str, Downloadable)> = Vec::new();

    targets.extend(
        app.get_addons(AddonType::Plugin)
            .into_iter()
            .map(|dl| ("Plugin", dl)),
    );
    targets.extend(
        app.get_addons(AddonType::Mod)
            .into_iter()
            .map(|dl| ("Mod", dl)),
    );

    if let ServerType::Downloadable { inner } = &app.server.jar {
        targets.push(("Server Jar", inner.clone()));
    }

    targets.retain(|(_, dl)| is_checkable(dl));

    if targets.is_empty() {
        app.info("No plugins, mods, or server jar pinned to Modrinth/Hangar/Spigot to check.");
        return Ok(());
    }

    let pb = app
        .multi_progress
        .add(
            ProgressBar::new(targets.len() as u64).with_style(ProgressStyle::with_template(
                "{msg} [{wide_bar:.cyan/blue}] {pos}/{len}",
            )?),
        );
    pb.set_message("Checking for updates");

    let mut table = MarkdownTable::with_headers(vec![
        Cow::Borrowed("Kind"),
        Cow::Borrowed("Addon"),
        Cow::Borrowed("Current"),
        Cow::Borrowed("Latest"),
    ]);

    for (kind, dl) in &targets {
        pb.set_message(format!("Checking {dl}"));

        match check_update(&app, dl, args.all_channels).await {
            Ok(CheckResult::Outdated { current, latest }) => {
                let mut row = IndexMap::new();
                row.insert(Cow::Borrowed("Kind"), (*kind).to_owned());
                row.insert(Cow::Borrowed("Addon"), dl.to_short_string());
                row.insert(Cow::Borrowed("Current"), current);
                row.insert(Cow::Borrowed("Latest"), latest);
                table.add_from_map(row);
            }
            Ok(CheckResult::UpToDate) => {}
            Err(err) => {
                app.warn(format!("Failed to check '{dl}' for updates: {err}"));
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();

    if table.rows.is_empty() {
        app.success("Everything is up to date!");
    } else {
        app.println(table.render_ascii());
        app.info(format!(
            "{} outdated - this only reports updates, edit server.toml to apply them",
            table.rows.len()
        ));
    }

    Ok(())
}
