use crate::tab::TabId;

pub enum Request {
    Connect(TabId),
    Terminate(TabId),
    ListTabs,
}
