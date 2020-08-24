use tab_service::{service_bus, Message};
use tokio::sync::broadcast;

service_bus!(pub TabBus);
