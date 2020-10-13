use std::io::Write;

use crate::{
    message::fuzzy::FuzzyEvent, message::fuzzy::FuzzyRecv, message::fuzzy::FuzzySelection,
    message::fuzzy::FuzzyShutdown, prelude::*, state::fuzzy::FuzzyMatch,
    state::fuzzy::FuzzyMatchState, state::fuzzy::FuzzyQueryState, state::fuzzy::FuzzySelectState,
    state::fuzzy::TabEntry,
};
use crossterm::{
    cursor::Hide,
    cursor::Show,
    style::{Colorize, Styler},
};
use crossterm::{
    cursor::MoveTo, execute, style::Print, style::PrintStyledContent, terminal::Clear,
    terminal::ClearType, QueueableCommand,
};
use crossterm::{event::Event, event::EventStream, event::KeyCode, terminal::enable_raw_mode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use tokio::{stream::Stream, stream::StreamExt, sync::watch};

pub struct FuzzyFinderService {
    _input: Lifeline,
    _query_state: Lifeline,
    _filter_state: Lifeline,
    _select_state: Lifeline,
    _select: Lifeline,
    _output: Lifeline,
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
            let rx = bus.rx::<FuzzyRecv>()?;
            let rx_query = bus.rx::<FuzzyQueryState>()?;
            let tx = bus.tx::<FuzzyMatchState>()?;
            Self::try_task("filter_state", Self::filter_state(rx, rx_query, tx))
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
                Self::send_selected(rx, rx_selection, tx, tx_shutdown),
            )
        };

        let _output = {
            let rx_query = bus.rx::<FuzzyQueryState>()?.into_inner();
            let rx_match = bus.rx::<FuzzyMatchState>()?.into_inner();
            let rx_select = bus.rx::<Option<FuzzySelectState>>()?.into_inner();
            Self::try_task("output", Self::output(rx_query, rx_match, rx_select))
        };

        Ok(Self {
            _input,
            _query_state,
            _filter_state,
            _select_state,
            _select,
            _output,
        })
    }
}

impl FuzzyFinderService {
    async fn input(
        mut tx_event: impl Sender<FuzzyEvent>,
        mut tx_shutdown: impl Sender<FuzzyShutdown>,
    ) -> anyhow::Result<()> {
        let mut reader = EventStream::new();

        while let Some(event) = reader.next().await {
            // println!("got event: {:?}", &event);
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
                            tx_event.send(FuzzyEvent::Insert(ch)).await?;
                        }
                        KeyCode::Esc => {
                            tx_shutdown.send(FuzzyShutdown {}).await.ok();

                            execute!(
                                std::io::stdout(),
                                MoveTo(0, 0),
                                Clear(ClearType::All),
                                MoveTo(0, 0)
                            )?;
                        }
                        KeyCode::Home => {}
                        KeyCode::End => {}
                        KeyCode::PageUp => {}
                        KeyCode::PageDown => {}
                        KeyCode::Tab => {}
                        KeyCode::BackTab => {}
                        KeyCode::Insert => {}
                        KeyCode::F(_) => {}
                        KeyCode::Null => {}
                    },
                    Event::Mouse(_mouse) => {}
                    Event::Resize(_, _) => {}
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
        mut rx: impl Receiver<FuzzyRecv>,
        mut rx_query: impl Receiver<FuzzyQueryState>,
        mut tx: impl Sender<FuzzyMatchState>,
    ) -> anyhow::Result<()> {
        let matcher = SkimMatcherV2::default();
        let mut last_query = None;

        if let Some(db) = rx.recv().await {
            let entries = TabEntry::build(db.tabs);

            while let Some(state) = rx_query.recv().await {
                if last_query.is_some() && last_query.as_ref().unwrap() == &state.query {
                    continue;
                }

                let mut matches = Vec::new();
                if state.query == "" {
                    for tab in entries.iter() {
                        matches.push(FuzzyMatch {
                            score: 0,
                            indices: Vec::new(),
                            tab: tab.clone(),
                        });
                    }

                    last_query = Some(state.query);
                    tx.send(FuzzyMatchState { matches }).await?;
                    continue;
                }

                let mut matches = Vec::new();
                for entry in entries.iter() {
                    let fuzzy_match =
                        matcher.fuzzy_indices(entry.display.as_str(), state.query.as_str());

                    if let Some((score, indices)) = fuzzy_match {
                        let tab_match = FuzzyMatch {
                            score,
                            indices,
                            tab: entry.clone(),
                        };

                        matches.push(tab_match);
                    }
                }

                matches.sort_by_key(|elem| elem.score);
                matches.reverse();

                last_query = Some(state.query);
                tx.send(FuzzyMatchState { matches }).await?;
            }
        }

        Ok(())
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

        while let Some(msg) = rx.next().await {
            match msg {
                Recv::Event(event) => match event {
                    FuzzyEvent::MoveUp => {
                        if index > 0 {
                            index -= 1;
                        }
                    }
                    FuzzyEvent::MoveDown => {
                        if index + 1 < matches.len() {
                            index += 1;
                        }
                    }
                    _ => {
                        continue;
                    }
                },
                Recv::Matches(message) => {
                    matches = message.matches;

                    if matches.len() == 0 {
                        index = 0;
                    } else if index > matches.len() - 1 {
                        index = matches.len() - 1;
                    }
                }
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

                    if let Some(name) = name {
                        tx.send(FuzzySelection(name)).await?;
                    } else {
                        tx_shutdown.send(FuzzyShutdown {}).await?;
                    }

                    execute!(
                        std::io::stdout(),
                        MoveTo(0, 0),
                        Clear(ClearType::All),
                        MoveTo(0, 0)
                    )?;

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

    async fn output(
        rx_query: watch::Receiver<FuzzyQueryState>,
        rx_match: watch::Receiver<FuzzyMatchState>,
        rx_select: watch::Receiver<Option<FuzzySelectState>>,
    ) -> anyhow::Result<()> {
        enable_raw_mode()?;

        enum Recv {
            Query(FuzzyQueryState),
            Matches(FuzzyMatchState),
            Select(Option<FuzzySelectState>),
        }

        let mut rx = rx_query
            .map(|q| Recv::Query(q))
            .merge(rx_match.map(|m| Recv::Matches(m)))
            .merge(rx_select.map(|s| Recv::Select(s)));

        let mut cursor_index: u16 = 0;
        let mut selected_row: Option<usize> = None;
        let mut saved_matches = None;
        let mut stdout = std::io::stdout();

        while let Some(recv) = rx.next().await {
            let terminal_size = crossterm::terminal::size()?;
            let terminal_height = terminal_size.0;

            stdout.queue(Hide)?;

            match recv {
                Recv::Query(query) => {
                    // translate into screen coordinates
                    cursor_index = query.cursor_index as u16 + 2;

                    stdout.queue(MoveTo(0, 0))?;
                    stdout.queue(Print("❯ "))?;
                    stdout.queue(Print(query.query.as_str()))?;
                    stdout.queue(Clear(ClearType::UntilNewLine))?;
                }
                Recv::Matches(matches) => {
                    let matches = matches.matches;

                    for (row, ref match_data) in (1..terminal_height).zip(matches.iter()) {
                        let tokens = Self::parse(match_data);

                        let selected = selected_row == Some(row as usize - 1);
                        stdout.queue(MoveTo(0, row))?;

                        if selected {
                            stdout.queue(PrintStyledContent("❯ ".blue()))?;
                            Self::print_selected_match(&mut stdout, tokens)?;
                        } else {
                            stdout.queue(Print("  "))?;
                            Self::print_match(&mut stdout, tokens)?;
                        }
                    }

                    stdout.queue(Clear(ClearType::FromCursorDown))?;
                    saved_matches = Some(matches);
                }
                Recv::Select(state) => {
                    if let Some(select) = state {
                        let new_row = select.index as u16 + 1;

                        if let Some(selected) = selected_row {
                            stdout.queue(MoveTo(0, (selected as u16) + 1))?;
                            stdout.queue(Print("  "))?;

                            if let Some(m) =
                                saved_matches.as_ref().map(|m| m.get(selected)).flatten()
                            {
                                let tokens = Self::parse(&m);
                                Self::print_match(&mut stdout, tokens)?;
                            }
                        }

                        stdout.queue(MoveTo(0, new_row))?;
                        stdout.queue(PrintStyledContent("❯ ".blue()))?;

                        if let Some(m) = saved_matches
                            .as_ref()
                            .map(|m| m.get(select.index as usize))
                            .flatten()
                        {
                            let tokens = Self::parse(&m);
                            Self::print_selected_match(&mut stdout, tokens)?;
                        }

                        selected_row = Some(select.index);
                    }
                }
            }

            stdout.queue(MoveTo(cursor_index, 0))?;
            stdout.queue(Show)?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn print_selected_match(
        stdout: &mut std::io::Stdout,
        tokens: Vec<Token>,
    ) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::UnmatchedTab(s) => stdout.queue(PrintStyledContent(s.bold().blue()))?,
                Token::MatchedTab(s) => {
                    stdout.queue(PrintStyledContent(s.bold().blue().underlined()))?
                }
                Token::Unmatched(s) => stdout.queue(PrintStyledContent(s.blue()))?,
                Token::Matched(s) => stdout.queue(PrintStyledContent(s.blue().underlined()))?,
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn print_match(stdout: &mut std::io::Stdout, tokens: Vec<Token>) -> anyhow::Result<()> {
        for token in tokens.into_iter() {
            match token {
                Token::UnmatchedTab(s) => stdout.queue(PrintStyledContent(s.bold()))?,
                Token::MatchedTab(s) => stdout.queue(PrintStyledContent(s.bold().underlined()))?,
                Token::Unmatched(s) => stdout.queue(Print(s.dark_grey()))?,
                Token::Matched(s) => stdout.queue(PrintStyledContent(s.grey().underlined()))?,
            };
        }

        stdout.queue(Clear(ClearType::UntilNewLine))?;

        Ok(())
    }

    fn parse<'a>(fuzzy_match: &'a FuzzyMatch) -> Vec<Token> {
        let mut ret = Vec::new();

        let mut next_match_iter = fuzzy_match.indices.iter().copied();
        let mut next_match = next_match_iter.next();
        let mut token = Token::UnmatchedTab("".to_string());
        let tab_len = fuzzy_match.tab.name.len();

        for (i, ch) in fuzzy_match.tab.display.char_indices() {
            while next_match.is_some() && next_match.unwrap() < i {
                next_match = next_match_iter.next();
            }

            let new_token = if i < tab_len {
                if next_match == Some(i) {
                    Token::MatchedTab(ch.to_string())
                } else {
                    Token::UnmatchedTab(ch.to_string())
                }
            } else {
                if next_match == Some(i) {
                    Token::Matched(ch.to_string())
                } else {
                    Token::Unmatched(ch.to_string())
                }
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
}

enum Token {
    UnmatchedTab(String),
    MatchedTab(String),
    Unmatched(String),
    Matched(String),
}

enum TokenJoin {
    Same(Token),
    Different(Token, Token),
}

impl Token {
    pub fn join(self, other: Token) -> TokenJoin {
        match (self, other) {
            (Token::UnmatchedTab(mut a), Token::UnmatchedTab(b)) => {
                a += b.as_str();
                TokenJoin::Same(Token::UnmatchedTab(a))
            }
            (Token::MatchedTab(mut a), Token::MatchedTab(b)) => {
                a += b.as_str();
                TokenJoin::Same(Token::MatchedTab(a))
            }
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
