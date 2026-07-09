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
    /// Search every Hangar channel (eg. Beta, Alpha, Snapshot) for updates, ignoring
    /// each plugin's `channels` setting (which otherwise defaults to `["Release"]`).
    #[arg(long)]
    all_channels: bool,
}

enum CheckTarget {
    Downloadable(Downloadable),
    /// The server jar, when it's a PaperMC-family project (paper/velocity/waterfall/folia)
    /// pinned to an explicit build, resolved through the Fill API.
    PaperBuild {
        project: String,
        build: String,
    },
}

impl CheckTarget {
    fn label(&self) -> String {
        match self {
            Self::Downloadable(dl) => dl.to_short_string(),
            Self::PaperBuild { project, .. } => format!("PaperMC:{project}"),
        }
    }
}

enum CheckResult {
    UpToDate,
    Outdated { current: String, latest: String },
}

/// Whether `check_update` supports this target: a Modrinth/Hangar/Spigot addon or a
/// PaperMC-family server jar, pinned to an explicit version/build rather than a
/// floating `"latest"` (which is already up to date by definition on every build).
fn is_checkable(target: &CheckTarget) -> bool {
    match target {
        CheckTarget::Downloadable(dl) => matches!(
            dl,
            Downloadable::Modrinth { version, .. }
            | Downloadable::Hangar { version, .. }
            | Downloadable::Spigot { version, .. }
                if version != "latest"
        ),
        CheckTarget::PaperBuild { build, .. } => build != "latest",
    }
}

async fn check_update(app: &App, target: &CheckTarget, all_channels: bool) -> Result<CheckResult> {
    Ok(match target {
        CheckTarget::Downloadable(Downloadable::Modrinth { id, version }) => {
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
        CheckTarget::Downloadable(Downloadable::Hangar {
            id,
            version,
            channels,
        }) => {
            let current = app.hangar().fetch_hangar_version(id, version).await?;
            let latest = if all_channels {
                app.hangar().fetch_newest_version_any_channel(id).await?
            } else {
                app.hangar()
                    .fetch_newest_version_in_channels(id, channels)
                    .await?
            };

            if current.name == latest.name {
                CheckResult::UpToDate
            } else {
                CheckResult::Outdated {
                    current: current.name,
                    latest: latest.name,
                }
            }
        }
        CheckTarget::Downloadable(Downloadable::Spigot { id, version }) => {
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
        CheckTarget::PaperBuild { project, build } => {
            let current = app
                .papermc()
                .fetch_build(project, app.mc_version(), build)
                .await?;
            let latest = app
                .papermc()
                .fetch_build(project, app.mc_version(), "latest")
                .await?;

            if current.id == latest.id {
                CheckResult::UpToDate
            } else {
                CheckResult::Outdated {
                    current: format!("build {}", current.id),
                    latest: format!("build {}", latest.id),
                }
            }
        }
        CheckTarget::Downloadable(_) => {
            unreachable!("check_update() called on a non-checkable downloadable")
        }
    })
}

/// Reports plugins, mods and the server jar (when pinned to Modrinth, Hangar, Spigot,
/// or a PaperMC-family build) that have a newer version available upstream. Read-only:
/// it never edits server.toml or the lockfile, it only prints what could be updated.
pub async fn run(app: App, args: Args) -> Result<()> {
    let mut targets: Vec<(&'static str, CheckTarget)> = Vec::new();

    targets.extend(
        app.get_addons(AddonType::Plugin)
            .into_iter()
            .map(|dl| ("Plugin", CheckTarget::Downloadable(dl))),
    );
    targets.extend(
        app.get_addons(AddonType::Mod)
            .into_iter()
            .map(|dl| ("Mod", CheckTarget::Downloadable(dl))),
    );

    match &app.server.jar {
        ServerType::Downloadable { inner } => {
            targets.push(("Server Jar", CheckTarget::Downloadable(inner.clone())));
        }
        ServerType::PaperMC { project, build } => {
            targets.push((
                "Server Jar",
                CheckTarget::PaperBuild {
                    project: project.clone(),
                    build: build.clone(),
                },
            ));
        }
        _ => {}
    }

    targets.retain(|(_, target)| is_checkable(target));

    if targets.is_empty() {
        app.info(
            "No plugins, mods, or server jar pinned to Modrinth/Hangar/Spigot/PaperMC to check.",
        );
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

    for (kind, target) in &targets {
        let label = target.label();
        pb.set_message(format!("Checking {label}"));

        match check_update(&app, target, args.all_channels).await {
            Ok(CheckResult::Outdated { current, latest }) => {
                let mut row = IndexMap::new();
                row.insert(Cow::Borrowed("Kind"), (*kind).to_owned());
                row.insert(Cow::Borrowed("Addon"), label);
                row.insert(Cow::Borrowed("Current"), current);
                row.insert(Cow::Borrowed("Latest"), latest);
                table.add_from_map(row);
            }
            Ok(CheckResult::UpToDate) => {}
            Err(err) => {
                app.warn(format!("Failed to check '{label}' for updates: {err}"));
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
