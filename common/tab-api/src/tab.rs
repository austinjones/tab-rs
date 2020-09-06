//! Common metadata about Tabs.

use lifeline::impl_storage_clone;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Identifies a running tab using a numeric index.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct TabId(pub u16);
impl_storage_clone!(TabId);

impl Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TabId(")?;
        self.0.fmt(f)?;
        f.write_str(")")?;

        Ok(())
    }
}

/// Tracked information about a running tab.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TabMetadata {
    pub id: TabId,
    pub name: String,
    pub dimensions: (u16, u16),
    pub shell: String,
    pub dir: String,
}

impl TabMetadata {
    pub fn create(id: TabId, create: CreateTabMetadata) -> Self {
        Self {
            id,
            name: create.name,
            dimensions: create.dimensions,
            shell: create.shell,
            dir: create.dir,
        }
    }
}

/// Information about a tab which will be created.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CreateTabMetadata {
    pub name: String,
    pub dimensions: (u16, u16),
    pub shell: String,
    pub dir: String,
}
