use futures_util::TryStreamExt;

use crate::commands::network::container::shared::{
    connect_to_docker, Error as ConnectionError, Network,
};

use super::shared::{get_container_name, ContainerArgs};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error("⛔ ️Failed to tail container: {0}")]
    TailContainerError(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: ContainerArgs,

    /// Network container to tail (used in container name generation)
    #[arg(required_unless_present = "container_name")]
    pub network: Option<Network>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name =
            get_container_name(self.container_args.container_name.clone(), self.network);
        let docker = connect_to_docker(&self.container_args.docker_host).await?;
        let logs_stream = &mut docker.logs(
            &container_name,
            Some(bollard::container::LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                tail: "all",
                ..Default::default()
            }),
        );

        while let Some(log) = logs_stream.try_next().await? {
            print!("{log}");
        }
        Ok(())
    }
}
