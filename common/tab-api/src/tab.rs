use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct TabId(pub u16);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TabMetadata {
    pub id: u16,
    pub name: String,
    pub dimensions: (u16, u16),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateTabMetadata {
    pub name: String,
    pub dimensions: (u16, u16),
}
