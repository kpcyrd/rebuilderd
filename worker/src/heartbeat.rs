use async_trait::async_trait;
use rebuilderd_common::errors::*;
use std::time::Duration;

#[async_trait]
pub trait HeartBeat {
    fn interval(&self) -> Duration;

    async fn ping(&self) -> Result<()>;
}
