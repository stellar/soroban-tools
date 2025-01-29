use crate::{
    commands::{container::shared::Error as BollardConnectionError, global},
    print,
};

use super::shared::{Args, Name};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ Failed to connect to docker: {0}")]
    DockerConnectionFailed(#[from] BollardConnectionError),

    #[error("⛔ Container {container_name} not found")]
    ContainerNotFound {
        container_name: String,
        #[source]
        source: bollard::errors::Error,
    },

    #[error("⛔ Failed to stop container: {0}")]
    ContainerStopFailed(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Container to stop
    #[arg(default_value = "local")]
    pub name: String,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let container_name = Name(self.name.clone());
        let docker = self.container_args.connect_to_docker(&print).await?;

        print.infoln(format!(
            "Stopping {} container",
            container_name.get_external_container_name()
        ));

        docker
            .stop_container(&container_name.get_internal_container_name(), None)
            .await
            .map_err(|e| {
                let msg = e.to_string();

                if msg.contains("No such container") {
                    Error::ContainerNotFound {
                        container_name: container_name.get_external_container_name(),
                        source: e,
                    }
                } else {
                    Error::ContainerStopFailed(e)
                }
            })?;

        print.checkln("Container stopped");

        Ok(())
    }
}
