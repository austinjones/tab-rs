use std::{fs::File, io::BufReader, io::Write, sync::Arc};

use crate::{
    env::terminal_size, message::fuzzy::FuzzyEvent, message::fuzzy::FuzzySelection,
    message::fuzzy::FuzzyShutdown, prelude::*, state::fuzzy::FuzzyMatch,
    state::fuzzy::FuzzyMatchState, state::fuzzy::FuzzyOutputEvent, state::fuzzy::FuzzyOutputMatch,
    state::fuzzy::FuzzyQueryState, state::fuzzy::FuzzySelectState, state::fuzzy::FuzzyTabsState,
    state::fuzzy::TabEntry, state::fuzzy::Token, state::fuzzy::TokenJoin,
};
use crossterm::{
    cursor::Hide,
    cursor::Show,
    event::KeyModifiers,
    style::{Colorize, Styler},
};
use crossterm::{
    cursor::MoveTo, execute, style::Print, style::PrintStyledContent, terminal::Clear,
    terminal::ClearType, QueueableCommand,
};
use crossterm::{event::Event, event::EventStream, event::KeyCode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use serde::Deserialize;
use tab_api::tab::normalize_name;
use tokio::{stream::Stream, stream::StreamExt, sync::watch};

/// Rows reserved by the UI for non-match items
const RESERVED_ROWS: usize = 2;

/// Columns reserved by the UI for non-match items
const RESERVED_COLUMNS: usize = 2;

pub struct FuzzyFinderService {
    _input: Lifeline,
    _query_state: Lifeline,
    _filter_state: Lifeline,
    _select_state: Lifeline,
    _select: Lifeline,
    _output_state: Lifeline,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyConfig {
    pub allow_new: bool,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self { allow_new: true }
    }
}

/// Parses the fuzzy config in the global workspace config file.
///
/// If the config file does not exists, returns the default config.
pub fn fuzzy_config() -> anyhow::Result<FuzzyConfig> {
    let config = tab_api::config::global_config_file();
    if let None = config {
        return Ok(FuzzyConfig::default());
    }

    let file = File::open(config.unwrap())?;
    let reader = BufReader::new(file);
    let fuzzy_config: FuzzyConfig = serde_yaml::from_reader(reader)?;
    Ok(fuzzy_config)
}

impl Service for FuzzyFinderService {
    type Bus = FuzzyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _input = {
            let tx = bus.tx::<FuzzyEvent>()?;
            let tx_shutdown = bus.tx::<FuzzyShutdown>()?;
            Self::try_task("input", Self::input(tx, tx_shutdown))
        };

        let _query_state = {
            let rx = bus.rx::<FuzzyEvent>()?;
            let tx = bus.tx::<FuzzyQueryState>()?;
            Self::try_task("query_state", Self::query_state(rx, tx))
        };

        let _filter_state = {
            let rx = bus.rx::<Option<FuzzyTabsState>>()?.into_inner();
            let rx_query = bus.rx::<FuzzyQueryState>()?.into_inner();
            let tx = bus.tx::<FuzzyMatchState>()?;
            let fuzzy_config = match fuzzy_config() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!(
                        "Warning: using default fuzzy config.  failed to parse global config: {}",
                        e
                    );
                    FuzzyConfig::default()
                }
            };

            Self::try_task(
                "filter_state",
                Self::filter_state(rx, rx_query, tx, fuzzy_config),
            )
        };

        let _select_state = {
            let rx = bus
                .rx::<FuzzyEvent>()?
                .into_inner()
                .filter(Result::is_ok)
                .map(Result::unwrap);
            let rx_matches = bus.rx::<FuzzyMatchState>()?.into_inner();
            let tx = bus.tx::<Option<FuzzySelectState>>()?;

            Self::try_task("select_state", Self::select_state(rx, rx_matches, tx))
        };

        let _output_state = {
            let rx_query = bus.rx::<FuzzyQueryState>()?.into_inner();
            let rx_match = bus.rx::<FuzzyMatchState>()?.into_inner();
            let rx_select = bus.rx::<Option<FuzzySelectState>>()?;
            let rx_event = bus
                .rx::<FuzzyEvent>()?
                .into_inner()
                .filter(|elem| elem.is_ok())
                .map(|elem| elem.unwrap());

            let tx = bus.tx::<FuzzyOutputEvent>()?;
            Self::try_task(
                "output_state",
                Self::output_state(rx_query, rx_match, rx_select, rx_event, tx),
            )
        };

        let _output = {
            let rx = bus.rx::<FuzzyOutputEvent>()?;
            Self::try_task("output", Self::output(rx))
        };

        let _select = {
            let rx = bus
                .rx::<FuzzyEvent>()?
                .into_inner()
                .filter(Result::is_ok)
                .map(Result::unwrap);
            let rx_selection = bus.rx::<Option<FuzzySelectState>>()?.into_inner();
            let tx = bus.tx::<FuzzySelection>()?;
            let tx_shutdown = bus.tx::<FuzzyShutdown>()?;

            Self::try_task(
                "send_selected",
                Self::send_selected(rx, rx_selection, tx, tx_shutdown, _output),
            )
        };

        Ok(Self {
            _input,
            _query_state,
            _filter_state,
            _select_state,
            _select,
            _output_state,
        })
    }
}

enum FilterEvent {
    Tabs(Option<FuzzyTabsState>),
    Query(FuzzyQueryState),
}

impl FuzzyFinderService {
    async fn input(
        mut tx_event: impl Sender<FuzzyEvent>,
        mut tx_shutdown: impl Sender<FuzzyShutdown>,
    ) -> anyhow::Result<()> {
        let mut reader = EventStream::new();

        let (cols, rows) = terminal_size()?;
        tx_event.send(FuzzyEvent::Resize(cols, rows)).await?;

        while let Some(event) = reader.next().await {
            if let Ok(event) = event {
                match event {
                    Event::Key(key) => match key.code {
                        KeyCode::Left => {
                            tx_event.send(FuzzyEvent::MoveLeft {}).await?;
                        }
                        KeyCode::Right => {
                            tx_event.send(FuzzyEvent::MoveRight {}).await?;
                        }
                        KeyCode::Up => {
                            tx_event.send(FuzzyEvent::MoveUp {}).await?;
                        }
                        KeyCode::Down => {
                            tx_event.send(FuzzyEvent::MoveDown {}).await?;
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            tx_event.send(FuzzyEvent::Delete {}).await?;
                        }
                        KeyCode::Enter => {
                            tx_event.send(FuzzyEvent::Enter).await?;
                        }
                        KeyCode::Char(ch) => {
                            if key.modifiers.eq(&KeyModifiers::CONTROL) && (ch == 'k' || ch == 'p')
                            {
                                tx_event.send(FuzzyEvent::MoveUp {}).await?;
                                continue;
                            }

                            if key.modifiers.eq(&KeyModifiers::CONTROL) && (ch == 'j' || ch == 'n')
                            {
                                tx_event.send(FuzzyEvent::MoveDown {}).await?;
                                continue;
                            }

                            if key.modifiers.eq(&KeyModifiers::CONTROL)
                                && (ch == 'c' || ch == 'x' || ch == 'w')
                            {
                                tx_shutdown.send(FuzzyShutdown {}).await.ok();
                                Self::clear_all()?;
                                continue;
                            }

                            if key.modifiers.eq(&KeyModifiers::CONTROL) {
                                continue;
                            }

                            tx_event.send(FuzzyEvent::Insert(ch)).await?;
                        }
                        KeyCode::Esc => {
                            tx_shutdown.send(FuzzyShutdown {}).await.ok();
                            Self::clear_all()?;
                        }
                        KeyCode::Home => {}
                        KeyCode::End => {}
                        KeyCode::PageUp => {
                            tx_event.send(FuzzyEvent::MoveFirst).await?;
                        }
                        KeyCode::PageDown => {
                            tx_event.send(FuzzyEvent::MoveLast).await?;
                        }
                        KeyCode::Tab => {
                            tx_event.send(FuzzyEvent::MoveDown).await?;
                        }
                        KeyCode::BackTab => {}
                        KeyCode::Insert => {}
                        KeyCode::F(_) => {}
                        KeyCode::Null => {}
                    },
                    Event::Mouse(_mouse) => {}
                    Event::Resize(cols, rows) => {
                        tx_event.send(FuzzyEvent::Resize(cols, rows)).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn query_state(
        mut rx: impl Receiver<FuzzyEvent>,
        mut tx: impl Sender<FuzzyQueryState>,
    ) -> anyhow::Result<()> {
        let mut query = "".to_string();
        let mut index = 0;

        while let Some(event) = rx.recv().await {
            match event {
                FuzzyEvent::MoveLeft => {
                    if index > 0 {
                        index -= 1;
                    }
                }
                FuzzyEvent::MoveRight => {
                    if index < query.len() {
                        index += 1;
                    }
                }
                FuzzyEvent::Insert(char) => {
                    query.insert(index, char);
                    index += 1;
                }
                FuzzyEvent::Delete => {
                    if index > 0 {
                        query.remove(index - 1);
                        index -= 1;
                    }
                }
                _ => {
                    continue;
                }
            }

            tx.send(FuzzyQueryState {
                query: query.clone(),
                cursor_index: index,
            })
            .await?;
        }

        Ok(())
    }

    async fn filter_state(
        rx: watch::Receiver<Option<FuzzyTabsState>>,
        rx_query: watch::Receiver<FuzzyQueryState>,
        mut tx: impl Sender<FuzzyMatchState>,
        fuzzy_config: FuzzyConfig,
    ) -> anyhow::Result<()> {
        let matcher = SkimMatcherV2::default().ignore_case();

        let mut rx = rx
            .map(|event| FilterEvent::Tabs(event))
            .merge(rx_query.map(|event| FilterEvent::Query(event)));

        let mut entries: Vec<Arc<TabEntry>> = vec![];
        let mut query = None;

        while let Some(event) = rx.next().await {
            match event {
                FilterEvent::Tabs(state) => {
                    if let Some(tabs) = state {
                        entries.clear();

                        for item in tabs.tabs.iter().map(TabEntry::from) {
                            entries.push(Arc::new(item));
                        }
                    }
                }
                FilterEvent::Query(state) => {
                    if query.is_some() && query.as_ref().unwrap() == &state.query {
                        continue;
                    }

                    if state.query.len() > 0 {
                        query = Some(state.query);
                    } else {
                        query = None;
                    }
                }
            }

            let create_entry =
                Self::create_tab_entry(&entries, &query, fuzzy_config.allow_new).map(Arc::new);

            let mut matches = Vec::new();
            let mut pattern = "".to_string();
            for entry in entries.iter().chain(create_entry.iter()) {
                if entry.sticky {
                    let name_len = entry.name.len();
                    matches.push(FuzzyMatch {
                        score: std::i64::MIN + 1,
                        name_indices: (0..name_len).collect(),
                        doc_indices: Vec::new(),
                        tab: entry.clone(),
                    });
                } else if let Some(ref query) = query {
                    pattern.clear();
                    pattern += &entry.name;

                    if let Some(ref doc) = entry.doc {
                        pattern += doc;
                    }

                    let fuzzy_match = matcher.fuzzy_indices(pattern.as_str(), query.as_str());

                    if let Some((score, indices)) = fuzzy_match {
                        let (name_indices, mut doc_indices): (Vec<usize>, Vec<usize>) =
                            indices.into_iter().partition(|e| *e < entry.name.len());

                        doc_indices.iter_mut().for_each(|e| *e -= entry.name.len());

                        let tab_match = FuzzyMatch {
                            score,
                            name_indices,
                            doc_indices,
                            tab: entry.clone(),
                        };

                        matches.push(tab_match);
                    }
                } else {
                    matches.push(FuzzyMatch {
                        score: std::i64::MIN + 1,
                        name_indices: Vec::new(),
                        doc_indices: Vec::new(),
                        tab: entry.clone(),
                    });
                }
            }

            matches.sort_by_key(|elem| -elem.score);

            tx.send(FuzzyMatchState {
                matches,
                total: entries.len() + create_entry.iter().count(),
            })
            .await?;
        }

        Ok(())
    }

    /// Creates a 'new tab' entry, if the user has entered a query, or entries is empty.
    /// Does not create an entry if the name conflicts with an element of entries
    fn create_tab_entry(
        entries: &Vec<Arc<TabEntry>>,
        query: &Option<String>,
        allow_new: bool,
    ) -> Option<TabEntry> {
        // if we don't have a query, and some entries exist, don't suggest a new tab.
        if entries.len() > 0 && query.is_none() {
            return None;
        }

        let name = query.as_ref().map(String::as_str).unwrap_or("tab");
        let name = normalize_name(name);

        if entries.iter().find(|tab| tab.name == name).is_some() || !allow_new {
            return None;
        }

        Some(TabEntry::entry_new(name.as_str()))
    }

    async fn select_state(
        rx: impl Stream<Item = FuzzyEvent> + Unpin,
        rx_matches: impl Stream<Item = FuzzyMatchState> + Unpin,
        mut tx: impl Sender<Option<FuzzySelectState>>,
    ) -> anyhow::Result<()> {
        enum Recv {
            Event(FuzzyEvent),
            Matches(FuzzyMatchState),
        }

        let mut rx = rx
            .map(|e| Recv::Event(e))
            .merge(rx_matches.map(|m| Recv::Matches(m)));

        let mut index: usize = 0;
        let mut matches: Vec<FuzzyMatch> = Vec::new();
        let mut terminal_height = terminal_size()?.1 as usize;

        while let Some(msg) = rx.next().await {
            match msg {
                Recv::Event(event) => match event {
                    FuzzyEvent::MoveUp => {
                        if index > 0 {
                            index -= 1;
                        }
                    }
                    FuzzyEvent::MoveDown => {
                        index += 1;
                    }
                    FuzzyEvent::MoveFirst => {
                        index = 0;
                    }
                    FuzzyEvent::MoveLast => {
                        if matches.len() > 0 {
                            index = matches.len() - 1;
                        }
                    }
                    FuzzyEvent::Resize(_rows, cols) => {
                        terminal_height = cols as usize;
                    }
                    _ => {
                        continue;
                    }
                },
                Recv::Matches(message) => {
                    matches = message.matches;
                }
            }

            if terminal_height < index + 1 + RESERVED_ROWS {
                index = terminal_height - 1 - RESERVED_ROWS;
            }

            if matches.len() == 0 {
                index = 0;
            } else if matches.len() <= index {
                index = matches.len() - 1
            }

            let state = matches
                .get(index)
                .map(|e| e.tab.clone())
                .map(|tab| FuzzySelectState { index, tab });

            tx.send(state).await?;
        }

        Ok(())
    }

    async fn send_selected(
        rx: impl Stream<Item = FuzzyEvent> + Unpin,
        rx_selection: impl Stream<Item = Option<FuzzySelectState>> + Unpin,
        mut tx: impl Sender<FuzzySelection>,
        mut tx_shutdown: impl Sender<FuzzyShutdown>,
        output: Lifeline,
    ) -> anyhow::Result<()> {
        #[derive(Debug)]
        enum Recv {
            Event(FuzzyEvent),
            Selection(Option<FuzzySelectState>),
        }

        let mut rx = rx
            .map(|q| Recv::Event(q))
            .merge(rx_selection.map(|m| Recv::Selection(m)));

        let mut selection: Option<FuzzySelectState> = None;

        while let Some(message) = rx.next().await {
            match message {
                Recv::Event(FuzzyEvent::Enter) => {
                    let name = selection.map(|state| state.tab.name.clone());

                    // cancel the output task
                    drop(output);

                    // then clear the terminal
                    Self::clear_all()?;

                    if let Some(name) = name {
                        tx.send(FuzzySelection(name)).await?;
                    } else {
                        tx_shutdown.send(FuzzyShutdown {}).await?;
                    }

                    break;
                }
                Recv::Selection(select_state) => {
                    selection = select_state;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn output_state(
        rx_query: impl Stream<Item = FuzzyQueryState> + Unpin,
        rx_match: impl Stream<Item = FuzzyMatchState> + Unpin,
        rx_select: impl Stream<Item = Option<FuzzySelectState>> + Unpin,
        rx_event: impl Stream<Item = FuzzyEvent> + Unpin,
        mut tx_state: impl Sender<FuzzyOutputEvent>,
    ) -> anyhow::Result<()> {
        let mut query_state = Arc::new(FuzzyQueryState::default());
        let mut match_state = Arc::new(vec![]);
        let mut total = 0usize;
        let mut doc_index = 4;
        let mut select_state = Arc::new(None);

        let mut rx = rx_query
            .map(|q| OutputRecv::Query(q))
            .merge(rx_match.map(|m| OutputRecv::Matches(m)))
            .merge(rx_select.map(|s| OutputRecv::Select(s)))
            .merge(rx_event.map(|s| OutputRecv::Event(s)));

        while let Some(msg) = rx.next().await {
            match msg {
                OutputRecv::Query(query) => {
                    query_state = Arc::new(query);
                }
                OutputRecv::Matches(matches) => {
                    total = matches.total;

                    doc_index = matches
                        .matches
                        .iter()
                        .map(|e| e.tab.name.len())
                        .max()
                        .map(|e| e + 2)
                        .unwrap_or(0)
                        .max(doc_index);

                    let matches: Vec<FuzzyOutputMatch> = matches
                        .matches
                        .into_iter()
                        .map(|mat| {
                            let (name, doc) = Self::parse(mat, doc_index);

                            FuzzyOutputMatch { name, doc }
                        })
                        .collect();

                    match_state = Arc::new(matches);
                }
                OutputRecv::Select(select) => {
                    select_state = Arc::new(select);
                }
                OutputRecv::Event(event) => match event {
                    FuzzyEvent::Resize(_cols, _rows) => {
                        // trigger render on resize
                    }
                    _ => continue,
                },
            }

            let event = FuzzyOutputEvent {
                query_state: query_state.clone(),
                select_state: select_state.clone(),
                matches: match_state.clone(),
                total,
            };

            tx_state.send(event).await.ok();
        }

        Ok(())
    }

    async fn output(mut rx: impl Receiver<FuzzyOutputEvent>) -> anyhow::Result<()> {
        let mut stdout = std::io::stdout();

        while let Some(state) = rx.recv().await {
            Self::draw(&mut stdout, state)?;
        }

        Ok(())
    }

    fn draw(stdout: &mut std::io::Stdout, state: FuzzyOutputEvent) -> anyhow::Result<()> {
        let query = state.query_state;
        let matches = state.matches;
        let selected = state.select_state;
        let selected_index = (*selected).as_ref().map(|elem| elem.index);

        let terminal_size = crossterm::terminal::size()?;
        let terminal_height = terminal_size.1;

        stdout.queue(Hide)?;

        stdout.queue(MoveTo(0, 0))?;
        stdout.queue(Print("❯ "))?;
        stdout.queue(Print(query.query.as_str().bold()))?;
        stdout.queue(Clear(ClearType::UntilNewLine))?;

        stdout.queue(MoveTo(0, 1))?;
        stdout.queue(Print("  "))?;
        stdout.queue(PrintStyledContent(matches.len().to_string().bold()))?;
        stdout.queue(PrintStyledContent("/".bold()))?;
        stdout.queue(PrintStyledContent(state.total.to_string().bold()))?;
        stdout.queue(Clear(ClearType::UntilNewLine))?;

        for (row, ref output_match) in (RESERVED_ROWS..terminal_height as usize).zip(matches.iter())
        {
            let name = &output_match.name;
            let doc = &output_match.doc;

            let selected = selected_index == Some(row - RESERVED_ROWS);
            stdout.queue(MoveTo(0, row as u16))?;

            if selected {
                stdout.queue(PrintStyledContent("❯ ".blue()))?;
                Self::print_selected_tab(stdout, name)?;

                if let Some(doc) = doc {
                    Self::print_selected_doc(stdout, doc)?;
                }
            } else {
                stdout.queue(Print("  "))?;
                Self::print_tab(stdout, name)?;

                if let Some(doc) = doc {
                    Self::print_doc(stdout, doc)?;
                }
            }
        }

        stdout.queue(Clear(ClearType::FromCursorDown))?;

        let cursor_index = query.cursor_index + RESERVED_COLUMNS;
        stdout.queue(MoveTo(cursor_index as u16, 0))?;
        stdout.queue(Show)?;
        stdout.flush()?;

        Ok(())
    }

    fn print_selected_tab(stdout: &mut std::io::Stdout, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::Unmatched(s) => {
                    stdout.queue(PrintStyledContent(s.as_str().bold().blue()))?
                }
                Token::Matched(s) => {
                    stdout.queue(PrintStyledContent(s.as_str().bold().blue().underlined()))?
                }
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn print_selected_doc(stdout: &mut std::io::Stdout, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::Unmatched(s) => stdout.queue(PrintStyledContent(s.as_str().blue()))?,
                Token::Matched(s) => {
                    stdout.queue(PrintStyledContent(s.as_str().blue().underlined()))?
                }
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn print_tab(stdout: &mut std::io::Stdout, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::Unmatched(s) => stdout.queue(PrintStyledContent(s.as_str().bold()))?,
                Token::Matched(s) => {
                    stdout.queue(PrintStyledContent(s.as_str().bold().underlined()))?
                }
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn print_doc(stdout: &mut std::io::Stdout, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::Unmatched(s) => stdout.queue(Print(s.as_str().dark_grey()))?,
                Token::Matched(s) => {
                    stdout.queue(PrintStyledContent(s.as_str().grey().underlined()))?
                }
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn parse(mat: FuzzyMatch, doc_index: usize) -> (Vec<Token>, Option<Vec<Token>>) {
        let mut name = mat.tab.name.clone();

        if name.len() < doc_index {
            name += " ".repeat(doc_index - name.len()).as_str();
        }

        let name = Self::parse_text(name.as_str(), &mat.name_indices);

        let doc = mat
            .tab
            .doc
            .as_ref()
            .map(|doc| Self::parse_text(doc.as_str(), &mat.doc_indices));

        (name, doc)
    }

    fn parse_text(text: &str, indices: &Vec<usize>) -> Vec<Token> {
        let mut ret = Vec::new();

        let mut next_match_iter = indices.into_iter().copied();
        let mut next_match = next_match_iter.next();
        let mut token = Token::Unmatched("".to_string());

        for (i, ch) in text.char_indices() {
            while next_match.is_some() && next_match.unwrap() < i {
                next_match = next_match_iter.next();
            }

            let new_token = if next_match == Some(i) {
                Token::Matched(ch.to_string())
            } else {
                Token::Unmatched(ch.to_string())
            };

            token = match token.join(new_token) {
                TokenJoin::Same(merged) => merged,
                TokenJoin::Different(prev, current) => {
                    ret.push(prev);
                    current
                }
            }
        }

        ret.push(token);

        ret
    }

    fn clear_all() -> anyhow::Result<()> {
        execute!(
            std::io::stdout(),
            MoveTo(0, 0),
            Clear(ClearType::All),
            MoveTo(0, 0)
        )?;

        Ok(())
    }
}

enum OutputRecv {
    Query(FuzzyQueryState),
    Matches(FuzzyMatchState),
    Select(Option<FuzzySelectState>),
    Event(FuzzyEvent),
}
