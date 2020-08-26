use lifeline::{impl_storage_clone, prelude::*};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TabMetadata {
    pub id: TabId,
    pub name: String,
    pub dimensions: (u16, u16),
}

impl TabMetadata {
    pub fn create(id: TabId, create: CreateTabMetadata) -> Self {
        Self {
            id,
            name: create.name,
            dimensions: create.dimensions,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateTabMetadata {
    pub name: String,
    pub dimensions: (u16, u16),
}
