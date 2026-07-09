use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::app::{App, CacheStrategy, Resolvable, ResolvedFile};

mod markdown;
mod meta;

#[derive(Debug, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Downloadable {
    // sources
    Url {
        url: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "crate::util::is_default")]
        filename: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "crate::util::is_default")]
        desc: Option<String>,
    },

    #[serde(alias = "mr")]
    Modrinth {
        id: String,
        #[serde(default = "latest")]
        version: String,
        /// Modrinth version channels considered by `mcman outdated` when looking for a
        /// newer version of this addon. Defaults to `["release"]`; add eg. `"beta"` or
        /// `"alpha"` to also be notified about pre-releases. Does not affect
        /// build/resolve behavior - only which channels count as "up to date" checking.
        #[serde(default = "default_modrinth_channels")]
        #[serde(skip_serializing_if = "is_default_modrinth_channels")]
        channels: Vec<String>,
    },

    #[serde(alias = "cr")]
    CurseRinth {
        id: String,
        #[serde(default = "latest")]
        version: String,
    },

    #[serde(alias = "cf")]
    CurseForge {
        id: String,
        #[serde(default = "latest")]
        version: String,
    },

    Spigot {
        id: String,
        #[serde(default = "latest")]
        version: String,
    },

    Hangar {
        id: String,
        version: String,
        /// Hangar update channels considered by `mcman outdated` when looking for a
        /// newer version of this plugin. Defaults to `["Release"]`; add eg. `"Beta"`
        /// to also be notified about pre-releases. Does not affect build/resolve
        /// behavior - only which channels count as "up to date" checking.
        #[serde(default = "default_hangar_channels")]
        #[serde(skip_serializing_if = "is_default_hangar_channels")]
        channels: Vec<String>,
    },

    #[serde(rename = "ghrel")]
    GithubRelease {
        repo: String,
        tag: String,
        asset: String,
    },

    // pain in the a-
    Jenkins {
        url: String,
        job: String,
        #[serde(default = "latest")]
        build: String,
        #[serde(default = "first")]
        artifact: String,
    },

    Maven {
        url: String,
        group: String,
        artifact: String,
        #[serde(default = "latest")]
        version: String,
        #[serde(default = "artifact")]
        filename: String,
    },
}

pub fn latest() -> String {
    "latest".to_owned()
}

pub fn first() -> String {
    "first".to_owned()
}

pub fn artifact() -> String {
    "artifact".to_owned()
}

pub fn default_hangar_channels() -> Vec<String> {
    vec![crate::sources::hangar::RELEASE_CHANNEL.to_owned()]
}

pub fn is_default_hangar_channels(channels: &[String]) -> bool {
    channels == default_hangar_channels()
}

pub fn default_modrinth_channels() -> Vec<String> {
    vec![crate::sources::modrinth::RELEASE_CHANNEL.to_owned()]
}

pub fn is_default_modrinth_channels(channels: &[String]) -> bool {
    channels == default_modrinth_channels()
}

impl Downloadable {
    /// Whether this spec asks for an explicit version/build, as opposed to a
    /// floating reference (eg. "latest"/"first") that must always be re-checked
    /// against the source no matter what the lockfile has cached.
    ///
    /// This alone does NOT mean a cached lockfile entry can be trusted without
    /// re-resolving: some sources (Spigot, Maven) never populate
    /// `ResolvedFile::hashes` even for an explicit version, and Jenkins only
    /// does when that particular build has fingerprinting enabled. Callers
    /// should not treat an entry as skip-eligible on `is_pinned()` alone —
    /// also check that the cached `ResolvedFile::hashes` is non-empty.
    pub fn is_pinned(&self) -> bool {
        match self {
            Self::Url { .. } | Self::Hangar { .. } => true,
            Self::GithubRelease { tag, .. } => tag != "latest",
            Self::Modrinth { version, .. }
            | Self::CurseRinth { version, .. }
            | Self::CurseForge { version, .. }
            | Self::Spigot { version, .. }
            | Self::Maven { version, .. } => version != "latest",
            Self::Jenkins {
                build, artifact, ..
            } => build != "latest" && artifact != "first",
        }
    }
}

impl Resolvable for Downloadable {
    async fn resolve_source(&self, app: &App) -> Result<ResolvedFile> {
        match self {
            Self::Url { url, filename, .. } => Ok(ResolvedFile {
                url: url.clone(),
                filename: if let Some(filename) = filename {
                    filename.clone()
                } else {
                    let url_clean = url.split('?').next().unwrap_or(url);
                    url_clean.split('/').next_back().unwrap().to_string()
                },
                cache: CacheStrategy::None,
                size: None,
                hashes: BTreeMap::new(),
            }),
            Self::Modrinth { id, version, .. } => app.modrinth().resolve_source(id, version).await,
            Self::CurseRinth { id, version } => app.curserinth().resolve_source(id, version).await,
            Self::CurseForge { id, version } => app.curseforge().resolve_source(id, version).await,
            Self::Spigot { id, version } => app.spigot().resolve_source(id, version).await,
            Self::Hangar { id, version, .. } => app.hangar().resolve_source(id, version).await,
            Self::GithubRelease { repo, tag, asset } => {
                app.github().resolve_source(repo, tag, asset).await
            }
            Self::Jenkins {
                url,
                job,
                build,
                artifact,
            } => {
                app.jenkins()
                    .resolve_source(url, job, build, artifact)
                    .await
            }
            Self::Maven {
                url,
                group,
                artifact,
                version,
                filename,
            } => {
                app.maven()
                    .resolve_source(url, group, artifact, version, filename)
                    .await
            }
        }
    }
}
