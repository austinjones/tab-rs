use std::sync::Arc;

use super::workspace::WorkspaceTab;

#[derive(Debug, Clone)]
pub struct FuzzyQueryState {
    pub query: String,
    pub cursor_index: usize,
}

impl Default for FuzzyQueryState {
    fn default() -> Self {
        Self {
            query: "".to_string(),
            cursor_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FuzzyMatchState {
    pub matches: Vec<FuzzyMatch>,
}

impl Default for FuzzyMatchState {
    fn default() -> Self {
        Self {
            matches: Vec::with_capacity(0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FuzzySelectState {
    pub index: usize,
    pub tab: Arc<TabEntry>,
}

#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub score: i64,
    pub indices: Vec<usize>,
    pub tab: Arc<TabEntry>,
}

#[derive(Debug, Clone)]
pub struct TabEntry {
    pub name: String,
    pub doc: Option<String>,
    pub display: String,
}

impl TabEntry {
    pub fn build(tabs: Vec<WorkspaceTab>) -> Vec<Arc<Self>> {
        let mut entries = Vec::with_capacity(tabs.len());
        let prefix_len = Self::tab_len(&tabs);

        for tab in tabs {
            let mut display = tab.name.clone();

            while display.len() < prefix_len {
                display += " ";
            }

            if let Some(ref doc) = tab.doc {
                display += "(";
                display += doc;
                display += ")";
            }

            let tab = Self {
                name: tab.name,
                doc: tab.doc,
                display,
            };

            entries.push(Arc::new(tab));
        }

        entries
    }

    fn tab_len(tabs: &Vec<WorkspaceTab>) -> usize {
        let max_len = tabs
            .iter()
            .map(|tab| tab.name.len())
            .max()
            .map(|len| len + 2);
        max_len.unwrap_or(0)
    }
}
