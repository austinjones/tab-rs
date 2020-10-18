use tab_api::tab::{CreateTabMetadata, TabId};

/// A message received by the `TabManagerService`, which manages the tab lifecycle and assigns tabs to PTY connections.
///
/// Carried over the `ListenerBus`
///
/// Usage:
/// - Rx from the `TabManagerService`, which creates & closes active tabs.
/// - Tx into the `ListenerConnectionCarrier`, to request that tabs be created/closed from a CLI connection.
/// - Tx into the `ListenerPtyCarrier`, to notify the manager that a PTY process is terminating (e.g. user typed `exit`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabManagerRecv {
    CreateTab(CreateTabMetadata),
    CloseTab(TabId),
}

/// A message sent by the `TabManagerService`, which notifies CLI connections of a closing tab.
///
/// Carried over the `ListenerBus`
///
/// Usage:
/// - Tx from the `TabManagerService` on tab lifecycle events
/// - Rx into the `ListenerConnectionCarrier`, to notify CLI connections of tab lifecycle events.
#[derive(Debug, Clone)]
pub enum TabManagerSend {
    TabTerminated(TabId),
}
