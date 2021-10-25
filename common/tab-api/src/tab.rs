//! Common metadata about Tabs.

use lifeline::impl_storage_clone;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, fmt::Display, num::ParseIntError, str::FromStr};

pub fn normalize_name(name: &str) -> String {
    let name = name.to_string().trim().to_string();
    if name.ends_with('/') {
        name
    } else {
        name + "/"
    }
}

pub fn validate_tab_name(name: String) -> Result<(), String> {
    if name.starts_with('-') {
        return Err("tab name may not begin with a dash".into());
    }

    if name.contains(' ') || name.contains('\t') || name.contains('\r') || name.contains('\n') {
        return Err("tab name may not contain whitespace".into());
    }

    if name.contains('\\') {
        return Err("tab name may not contain backslashes".into());
    }

    Ok(())
}

/// Identifies a running tab using a numeric index.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct TabId(pub u16);
impl_storage_clone!(TabId);

impl FromStr for TabId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = u16::from_str(s)?;
        Ok(Self(id))
    }
}

impl Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TabId(")?;
        self.0.fmt(f)?;
        f.write_str(")")?;

        Ok(())
    }
}

fn unix_time() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis())
        .unwrap_or(0)
}

/// Tracked information about a running tab.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TabMetadata {
    pub id: TabId,
    pub name: String,
    pub doc: Option<String>,
    pub dimensions: (u16, u16),
    pub env: HashMap<String, String>,
    pub shell: String,
    pub dir: String,
    pub selected: u128,
}

impl TabMetadata {
    pub fn create(id: TabId, create: CreateTabMetadata) -> Self {
        Self {
            id,
            name: create.name,
            doc: create.doc,
            dimensions: create.dimensions,
            env: create.env,
            shell: create.shell,
            dir: create.dir,
            selected: unix_time(),
        }
    }

    pub fn mark_selected(&mut self) {
        self.selected = unix_time();
    }
}

/// Information about a tab which will be created.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CreateTabMetadata {
    pub name: String,
    pub dimensions: (u16, u16),
    pub doc: Option<String>,
    pub env: HashMap<String, String>,
    pub shell: String,
    pub dir: String,
}
