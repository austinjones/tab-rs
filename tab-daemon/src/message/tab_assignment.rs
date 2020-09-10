use tab_api::tab::TabMetadata;

use crate::state::assignment::Retraction;

#[derive(Clone, Debug)]
pub struct AssignTab(pub TabMetadata);

#[derive(Debug, Clone)]
pub struct TabAssignmentRetraction(pub Retraction<TabMetadata>);
