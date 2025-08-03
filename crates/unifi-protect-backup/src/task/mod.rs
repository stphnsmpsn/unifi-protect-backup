use crate::Result;

mod archiver;
mod db_poller;
mod pruner;
mod unifi_event_listener;

pub use archiver::*;
pub use db_poller::*;
pub use pruner::*;
pub use unifi_event_listener::*;

#[async_trait::async_trait]
pub trait Prune {
    async fn prune(&self) -> Result<()>;
}
