use super::section::{Field, SettingsSection, TableRead, TableWrite};
use std::collections::HashSet;
use toml_edit::{Array, Item, Table, TableLike, Value, value};

/// A named R repository, such as `CRAN`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RRepository {
    pub name: String,
    pub url: String,
}

/// A Julia registry or Python package index URL.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackageIndex {
    pub url: String,
}

/// `[repositories]`, where deployments install packages from.
/// Ricochet reads the R repositories in name order, so they are saved and shown in name order.
#[derive(Clone, Debug, PartialEq)]
pub struct PackageSettings {
    pub r: Vec<RRepository>,
    pub julia: Vec<PackageIndex>,
    pub python: Vec<PackageIndex>,
    pub allow_overrides: bool,
}

impl Default for PackageSettings {
    fn default() -> Self {
        Self {
            r: Vec::new(),
            julia: Vec::new(),
            python: Vec::new(),
            allow_overrides: true,
        }
    }
}

fn indexes(table: &dyn TableLike, key: &str) -> Vec<PackageIndex> {
    table
        .strings(key)
        .unwrap_or_default()
        .into_iter()
        .map(|url| PackageIndex { url })
        .collect()
}

fn index_array(indexes: &[PackageIndex]) -> Option<Item> {
    (!indexes.is_empty()).then(|| {
        let urls: Array = indexes
            .iter()
            .map(|index| Value::from(index.url.trimmed()))
            .collect();
        value(urls)
    })
}

impl SettingsSection for PackageSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let Some(repositories) = table.child("repositories") else {
            return Self::default();
        };
        let mut r: Vec<RRepository> = repositories
            .child("r")
            .map(|named| {
                named
                    .iter()
                    .map(|(name, url)| RRepository {
                        name: name.to_owned(),
                        url: url.as_str().unwrap_or("").to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        r.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            r,
            julia: indexes(repositories, "julia"),
            python: indexes(repositories, "python"),
            allow_overrides: repositories.boolean("allow_overrides").unwrap_or(true),
        }
    }

    fn apply(&self, table: &mut Table) {
        let repositories = table.section("repositories");
        let mut named = Table::new();
        let mut sorted: Vec<&RRepository> = self.r.iter().collect();
        sorted.sort_by(|a, b| a.name.trimmed().cmp(b.name.trimmed()));
        for repository in sorted {
            named.insert(repository.name.trimmed(), value(repository.url.trimmed()));
        }
        if named.is_empty() {
            repositories.remove("r");
        } else {
            repositories.insert("r", Item::Table(named));
        }
        for (key, list) in [("julia", &self.julia), ("python", &self.python)] {
            match index_array(list) {
                Some(array) => {
                    repositories.insert(key, array);
                }
                None => {
                    repositories.remove(key);
                }
            }
        }
        repositories.set("allow_overrides", (!self.allow_overrides).then_some(false));
        table.remove_if_empty("repositories");
    }

    fn problem(&self) -> Option<String> {
        if self
            .r
            .iter()
            .any(|repository| repository.name.non_empty().is_none())
        {
            return Some("Name every R repository, such as CRAN.".into());
        }
        let names: HashSet<&str> = self
            .r
            .iter()
            .map(|repository| repository.name.trimmed())
            .collect();
        if names.len() != self.r.len() {
            return Some("Give each R repository a different name.".into());
        }
        let any_bad = self
            .r
            .iter()
            .map(|repository| &repository.url)
            .chain(self.julia.iter().map(|index| &index.url))
            .chain(self.python.iter().map(|index| &index.url))
            .any(|url| !url.is_http_url());
        if any_bad {
            return Some("Start every repository URL with https://.".into());
        }
        None
    }
}
