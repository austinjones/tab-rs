use std::sync::Arc;

use tab_api::tab::normalize_name;

use super::workspace::{WorkspaceState, WorkspaceTab};
#[derive(Debug, Clone)]
pub struct FuzzyTabsState {
    pub tabs: Arc<Vec<WorkspaceTab>>,
}

impl From<WorkspaceState> for FuzzyTabsState {
    fn from(workspace: WorkspaceState) -> Self {
        Self {
            tabs: workspace.tabs,
        }
    }
}

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
    pub total: usize,
    pub matches: Vec<FuzzyMatch>,
}

impl Default for FuzzyMatchState {
    fn default() -> Self {
        Self {
            total: 0,
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
    pub name_indices: Vec<usize>,
    pub doc_indices: Vec<usize>,
    pub tab: Arc<TabEntry>,
}

#[derive(Debug, Clone)]
pub struct FuzzyOutputEvent {
    pub query_state: Arc<FuzzyQueryState>,
    pub select_state: Arc<Option<FuzzySelectState>>,
    pub matches: Arc<Vec<FuzzyOutputMatch>>,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct FuzzyOutputMatch {
    pub name: Vec<Token>,
    pub doc: Option<Vec<Token>>,
}

#[derive(Debug, Clone)]
pub struct FuzzyEntryState {
    pub entries: Vec<Arc<TabEntry>>,
}

#[derive(Debug, Clone)]
pub struct TabEntry {
    pub name: String,
    pub doc: Option<String>,
    pub last_selected: Option<u128>,
    pub sticky: bool,
}

impl From<&WorkspaceTab> for TabEntry {
    fn from(tab: &WorkspaceTab) -> Self {
        Self {
            name: tab.name.clone(),
            doc: tab.doc.clone().map(|mut doc| {
                doc.insert(0, '(');
                doc.push(')');
                doc
            }),
            last_selected: tab.last_selected.clone(),
            sticky: false,
        }
    }
}

impl TabEntry {
    pub fn entry_new(query: &str) -> TabEntry {
        let name = normalize_name(query);
        let doc = "(new tab)";

        TabEntry {
            name,
            doc: Some(doc.to_string()),
            sticky: true,
            last_selected: None,
        }
    }

    pub fn entry_tutorial() -> TabEntry {
        let name = "tab/";
        let doc = "(write a tab name to get started, or press enter to use this one)";

        TabEntry {
            name: name.to_string(),
            doc: Some(doc.to_string()),
            sticky: true,
            last_selected: None,
        }
    }

    pub fn display(&self, doc_index: usize) -> String {
        let mut display = self.name.to_string();

        while display.len() < doc_index {
            display += " ";
        }

        if let Some(ref doc) = self.doc {
            display += "(";
            display += doc;
            display += ")";
        }

        display
    }

    pub fn tab_len<'a>(tabs: impl Iterator<Item = &'a Self>) -> usize {
        let max_len = tabs.map(|tab| tab.name.len()).max().map(|len| len + 2);
        max_len.unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    Unmatched(String),
    Matched(String),
}

pub enum TokenJoin {
    Same(Token),
    Different(Token, Token),
}

impl Token {
    pub fn join(self, other: Token) -> TokenJoin {
        match (self, other) {
            (Token::Unmatched(mut a), Token::Unmatched(b)) => {
                a += b.as_str();
                TokenJoin::Same(Token::Unmatched(a))
            }
            (Token::Matched(mut a), Token::Matched(b)) => {
                a += b.as_str();
                TokenJoin::Same(Token::Matched(a))
            }
            (s, o) => TokenJoin::Different(s, o),
        }
    }
}
