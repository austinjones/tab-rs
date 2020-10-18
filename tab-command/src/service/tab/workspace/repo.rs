use std::path::Path;

use tab_api::tab::normalize_name;

use crate::state::workspace::{Repo, WorkspaceTab};

use super::loader::TabIter;

pub fn repo_iter(path: &Path, repo: Repo) -> TabIter {
    let mut iter = TabIter::new();

    let repo_name = normalize_name(repo.repo.as_str());

    // push a tab for the repo
    let tab = WorkspaceTab::with_options(
        repo_name.as_str(),
        path.to_path_buf(),
        repo.tab_options.clone(),
    );
    iter.and(tab);

    // and then for any tabs the user defined
    for tab in repo.tabs.into_iter().flat_map(|t| t.into_iter()) {
        let mut directory = path.to_path_buf();
        if let Some(subdir) = tab.dir {
            directory.push(subdir);
        }

        let tab_name = normalize_name(tab.tab.as_str());
        let tab_name = repo_name.clone() + tab_name.as_str();

        let options = tab.options.or(repo.tab_options.clone());

        let tab = WorkspaceTab::with_options(tab_name.as_str(), directory, options);
        iter.and(tab);
    }

    iter
}
