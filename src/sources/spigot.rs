use std::{borrow::Cow, collections::BTreeMap};

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::app::{App, CacheStrategy, ResolvedFile};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpigotVersion {
    pub uuid: String,
    pub name: String,
    pub resource: u64,
    pub id: u64,
}

pub struct SpigotAPI<'a>(pub &'a App);

pub const API_URL: &str = "https://api.spiget.org/v2";
pub const CACHE_DIR: &str = "spiget";

// Spiget doesn't let you look up a version by its (non-unique) name directly,
// so we have to page through the full version list ourselves to find it.
const VERSIONS_PAGE_SIZE: u32 = 100;

impl SpigotAPI<'_> {
    pub async fn fetch_api<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response: T = self
            .0
            .http_client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(response)
    }

    pub fn get_resource_id(res: &str) -> &str {
        if let Some(i) = res.find('.') {
            if i < res.len() - 1 {
                return res.split_at(i + 1).1;
            }
        }

        res
    }

    pub async fn fetch_info(&self, id: &str) -> Result<(String, String)> {
        let json = self
            .fetch_api::<serde_json::Value>(&format!(
                "{API_URL}/resources/{}",
                Self::get_resource_id(id)
            ))
            .await?;

        Ok((
            json["name"].as_str().unwrap().to_owned(),
            json["tag"].as_str().unwrap().to_owned(),
        ))
    }

    /// Fetches a single page of a resource's version list.
    ///
    /// Returns the versions on that page along with the total page count, read
    /// from Spiget's `X-Page-Count` response header (the versions endpoint doesn't
    /// include pagination info in its JSON body).
    pub async fn fetch_versions_page(
        &self,
        id: &str,
        page: u32,
        size: u32,
    ) -> Result<(Vec<SpigotVersion>, u32)> {
        let response = self
            .0
            .http_client
            .get(format!(
                "{API_URL}/resources/{}/versions",
                Self::get_resource_id(id)
            ))
            .query(&[("size", size), ("page", page)])
            .send()
            .await?
            .error_for_status()?;

        let page_count = response
            .headers()
            .get("x-page-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);

        let versions = response.json::<Vec<SpigotVersion>>().await?;

        Ok((versions, page_count))
    }

    /// Fetches every version of a resource, paging through Spiget's version list.
    #[allow(unused)]
    pub async fn fetch_versions(&self, id: &str) -> Result<Vec<SpigotVersion>> {
        let mut versions = Vec::new();
        let mut page = 1;

        loop {
            let (mut page_versions, page_count) = self
                .fetch_versions_page(id, page, VERSIONS_PAGE_SIZE)
                .await?;
            versions.append(&mut page_versions);

            if page >= page_count {
                break;
            }
            page += 1;
        }

        Ok(versions)
    }

    /// Fetches a version by its internal Spiget id, or the resource's latest version if `id_or_latest` is `"latest"`.
    pub async fn fetch_version_by_id(&self, id: &str, id_or_latest: &str) -> Result<SpigotVersion> {
        self.fetch_api(&format!(
            "{API_URL}/resources/{}/versions/{id_or_latest}",
            Self::get_resource_id(id)
        ))
        .await
    }

    /// Looks up a version by its display name (eg. `"1.4.1"`), since Spiget only
    /// supports looking versions up by internal id or the literal `"latest"`.
    pub async fn fetch_version_by_name(&self, id: &str, name: &str) -> Result<SpigotVersion> {
        let mut page = 1;

        loop {
            let (page_versions, page_count) = self
                .fetch_versions_page(id, page, VERSIONS_PAGE_SIZE)
                .await?;

            if let Some(version) = page_versions.into_iter().find(|v| v.name == name) {
                return Ok(version);
            }

            if page >= page_count {
                break;
            }
            page += 1;
        }

        Err(anyhow!(
            "No version named '{name}' found for spigot resource '{id}'"
        ))
    }

    pub async fn fetch_version(&self, id: &str, version: &str) -> Result<SpigotVersion> {
        if version == "latest" {
            self.fetch_version_by_id(id, "latest").await
        } else {
            self.fetch_version_by_name(id, version).await
        }
    }

    pub async fn resolve_source(&self, id: &str, version: &str) -> Result<ResolvedFile> {
        let resolved_version = self.fetch_version(id, version).await?;

        let filename = format!("spigot-{id}-{}.jar", resolved_version.name);
        let cached_file_path = format!("{id}/{}.jar", resolved_version.id);

        Ok(ResolvedFile {
            url: format!(
                "{API_URL}/resources/{}/versions/{}/download/proxy",
                Self::get_resource_id(id),
                resolved_version.id
            ),
            filename,
            cache: CacheStrategy::File {
                namespace: Cow::Borrowed(CACHE_DIR),
                path: cached_file_path,
            },
            size: None,
            hashes: BTreeMap::new(),
        })
    }
}
