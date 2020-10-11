use std::{io::Write, sync::Arc};

use crate::{
    message::fuzzy::FuzzyEvent, message::fuzzy::FuzzyInterfaceRecv, message::fuzzy::FuzzyRecv,
    message::fuzzy::FuzzyShutdown, prelude::*, state::fuzzy::FuzzyMatch,
    state::fuzzy::FuzzyMatchState, state::fuzzy::FuzzyQueryState, state::fuzzy::TabEntry,
};
use crossterm::{
    cursor::{MoveLeft, MoveRight, MoveTo},
    execute,
    style::Print,
    terminal::Clear,
    terminal::ClearType,
};
use crossterm::{event::Event, event::EventStream, event::KeyCode, terminal::enable_raw_mode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use tokio::{stream::StreamExt, sync::watch};

pub struct FuzzyFinderService {
    _input: Lifeline,
    _query_state: Lifeline,
    _filter_state: Lifeline,
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

        let _output = {
            let rx_query = bus.rx::<FuzzyQueryState>()?.into_inner();
            let rx_match = bus.rx::<FuzzyMatchState>()?.into_inner();
            Self::try_task("output", Self::output(rx_query, rx_match))
        };

        Ok(Self {
            _input,
            _query_state,
            _filter_state,
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
                    Event::Mouse(mouse) => {}
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
                _ => {}
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

    async fn output(
        rx_query: watch::Receiver<FuzzyQueryState>,
        rx_match: watch::Receiver<FuzzyMatchState>,
    ) -> anyhow::Result<()> {
        enable_raw_mode()?;

        enum Recv {
            Query(FuzzyQueryState),
            Matches(FuzzyMatchState),
        }

        let mut rx = rx_query
            .map(|q| Recv::Query(q))
            .merge(rx_match.map(|m| Recv::Matches(m)));

        let mut cursor_index: u16 = 0;

        while let Some(recv) = rx.next().await {
            let terminal_size = crossterm::terminal::size()?;
            let terminal_height = terminal_size.0;

            match recv {
                Recv::Query(query) => {
                    cursor_index = query.cursor_index as u16 + 2;

                    execute!(
                        std::io::stdout(),
                        MoveTo(0, 0),
                        Print("> "),
                        Print(query.query.as_str()),
                        Clear(ClearType::UntilNewLine),
                    )?;
                }
                Recv::Matches(matches) => {
                    let matches = matches.matches;

                    for (row, match_data) in (1..terminal_height).zip(matches.into_iter()) {
                        execute!(
                            std::io::stdout(),
                            MoveTo(0, row),
                            Print("  "),
                            Print(match_data.tab.display.as_str()),
                            Clear(ClearType::UntilNewLine)
                        )?;
                    }

                    execute!(std::io::stdout(), Clear(ClearType::FromCursorDown),)?;
                }
            }

            execute!(std::io::stdout(), MoveTo(cursor_index, 0))?;
            // println!("got cursor index: {}", cursor_index);
        }

        // if let Some(tabs) = rx.recv().await {
        //     enable_raw_mode()?;

        //     while let Some(state) = rx_state.recv().await {
        //         match state {
        //             FuzzyQueryState::Query {
        //                 query,
        //                 cursor_index,
        //             } => {}
        //             FuzzyQueryState::Selected => {}
        //         }
        //         print!(">  {}", state.);
        //         print!();

        //         for tab in tabs.tabs {
        //             println!("\r{} ({})", tab.0, tab.1);
        //         }
        //     }
        // }

        Ok(())
    }
}
