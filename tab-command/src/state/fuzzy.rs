use std::sync::Arc;

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
pub struct FuzzyMatch {
    pub score: i64,
    pub indices: Vec<usize>,
    pub tab: Arc<TabEntry>,
}

#[derive(Debug, Clone)]
pub struct TabEntry {
    pub name: String,
    pub doc: String,
    pub display: String,
}

impl TabEntry {
    pub fn build(tabs: Vec<(String, String)>) -> Vec<Arc<Self>> {
        let mut entries = Vec::with_capacity(tabs.len());
        let prefix_len = Self::tab_len(&tabs);

        for (name, doc) in tabs {
            let mut display = name.clone();

            while display.len() < prefix_len {
                display += " ";
            }
            display += "(";
            display += &doc;
            display += ")";

            let tab = Self { name, doc, display };

            entries.push(Arc::new(tab));
        }

        entries
    }

    fn tab_len(tabs: &Vec<(String, String)>) -> usize {
        let max_len = tabs.iter().map(|tab| tab.0.len()).max();
        max_len.unwrap_or(0)
    }
}
