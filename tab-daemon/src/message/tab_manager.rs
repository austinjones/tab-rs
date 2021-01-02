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
    UpdateTimestamp(TabId),
    CloseTab(TabId),
}
