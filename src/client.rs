use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::sync::MutexGuard;

use crate::{
    core::{
        base_client::{BaseClient, DataCenter},
        network::{Response, UniRequest},
    },
    error::Error,
};

#[derive(Debug)]
pub struct Client {
    base_client: BaseClient,
    data_dir: PathBuf,
}

impl Client {
    pub async fn data(&self) -> MutexGuard<DataCenter> {
        self.base_client.data().await
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub async fn send_registered_request(
        &self,
        request: UniRequest,
        timeout: Option<Duration>,
    ) -> Result<Response, Error> {
        self.base_client
            .send_registered_request(request, timeout)
            .await
    }
}
