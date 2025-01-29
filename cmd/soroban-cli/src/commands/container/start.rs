use std::collections::HashMap;

use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
};
use futures_util::TryStreamExt;

use crate::{
    commands::{
        container::shared::{Error as ConnectionError, Network},
        global,
    },
    print,
};

use super::shared::{Args, Name};

const DEFAULT_PORT_MAPPING: &str = "8000:8000";
const DOCKER_IMAGE: &str = "docker.io/stellar/quickstart";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ ️Failed to connect to docker: {0}")]
    DockerConnectionFailed(#[from] ConnectionError),

    #[error("⛔ ️Failed to create container: {0}")]
    CreateContainerFailed(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Network to start. Default is `local`
    pub network: Option<Network>,

    /// Optional argument to specify the container name
    #[arg(long)]
    pub name: Option<String>,

    /// Optional argument to specify the limits for the local network only
    #[arg(short = 'l', long)]
    pub limits: Option<String>,

    /// Argument to specify the `HOST_PORT:CONTAINER_PORT` mapping
    #[arg(short = 'p', long, num_args = 1.., default_value = DEFAULT_PORT_MAPPING)]
    pub ports_mapping: Vec<String>,

    /// Optional argument to override the default docker image tag for the given network
    #[arg(short = 't', long)]
    pub image_tag_override: Option<String>,

    /// Optional argument to specify the protocol version for the local network only
    #[arg(long)]
    pub protocol_version: Option<String>,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let runner = Runner {
            args: self.clone(),
            network: self.network.unwrap_or(Network::Local),
            print: print::Print::new(global_args.quiet),
        };

        runner.run_docker_command().await
    }
}

struct Runner {
    args: Cmd,
    network: Network,
    print: print::Print,
}

impl Runner {
    async fn run_docker_command(&self) -> Result<(), Error> {
        self.print
            .infoln(format!("Starting {} network", &self.network));

        let docker = self
            .args
            .container_args
            .connect_to_docker(&self.print)
            .await?;

        let image = self.get_image_name();
        let mut stream = docker.create_image(
            Some(CreateImageOptions {
                from_image: image.clone(),
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.try_next().await.transpose() {
            if let Ok(item) = result {
                if let Some(status) = item.status {
                    if status.contains("Pulling from")
                        || status.contains("Digest")
                        || status.contains("Status")
                    {
                        self.print.infoln(status);
                    }
                }
            } else {
                self.print
                    .warnln(format!("Failed to fetch image: {image}."));
                self.print.warnln(
                    "Attempting to start local quickstart image. The image may be out-of-date.",
                );
                break;
            }
        }

        let config = Config {
            image: Some(image),
            cmd: Some(self.get_container_args()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            host_config: Some(HostConfig {
                auto_remove: Some(true),
                port_bindings: Some(self.get_port_mapping()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let create_container_response = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: self.container_name().get_internal_container_name(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        docker
            .start_container(
                &create_container_response.id,
                None::<StartContainerOptions<String>>,
            )
            .await?;
        self.print.checkln("Started container");
        self.print_instructions();
        Ok(())
    }

    fn get_image_name(&self) -> String {
        // this can be overriden with the `-t` flag
        let mut image_tag = match &self.network {
            Network::Pubnet => "latest",
            Network::Futurenet => "future",
            _ => "testing", // default to testing for local and testnet
        };

        if let Some(image_override) = &self.args.image_tag_override {
            self.print.infoln(format!(
                "Overriding docker image tag to use '{image_override}' instead of '{image_tag}'"
            ));
            image_tag = image_override;
        }

        format!("{DOCKER_IMAGE}:{image_tag}")
    }

    fn get_container_args(&self) -> Vec<String> {
        [
            format!("--{}", self.network),
            "--enable rpc,horizon".to_string(),
            self.get_protocol_version_arg(),
            self.get_limits_arg(),
        ]
        .iter()
        .filter(|&s| !s.is_empty())
        .cloned()
        .collect()
    }

    // The port mapping in the bollard crate is formatted differently than the docker CLI. In the docker CLI, we usually specify exposed ports as `-p  HOST_PORT:CONTAINER_PORT`. But with the bollard crate, it is expecting the port mapping to be a map of the container port (with the protocol) to the host port.
    fn get_port_mapping(&self) -> HashMap<String, Option<Vec<PortBinding>>> {
        let mut port_mapping_hash = HashMap::new();
        for port_mapping in &self.args.ports_mapping {
            let ports_vec: Vec<&str> = port_mapping.split(':').collect();
            let from_port = ports_vec[0];
            let to_port = ports_vec[1];

            port_mapping_hash.insert(
                format!("{to_port}/tcp"),
                Some(vec![PortBinding {
                    host_ip: None,
                    host_port: Some(from_port.to_string()),
                }]),
            );
        }

        port_mapping_hash
    }

    fn container_name(&self) -> Name {
        Name(self.args.name.clone().unwrap_or(self.network.to_string()))
    }

    fn print_instructions(&self) {
        let container_name = self.container_name().get_external_container_name().clone();
        let additional_flags = self.args.container_args.get_additional_flags().clone();
        let tail = format!("{container_name} {additional_flags}");

        self.print.searchln(format!(
            "Watch logs with `stellar network container logs {}`",
            tail.trim()
        ));

        self.print.infoln(format!(
            "Stop the container with `stellar network container stop {}`",
            tail.trim()
        ));
    }

    fn get_protocol_version_arg(&self) -> String {
        if self.network == Network::Local && self.args.protocol_version.is_some() {
            let version = self.args.protocol_version.as_ref().unwrap();
            format!("--protocol-version {version}")
        } else {
            String::new()
        }
    }

    fn get_limits_arg(&self) -> String {
        if self.network == Network::Local && self.args.limits.is_some() {
            let limits = self.args.limits.as_ref().unwrap();
            format!("--limits {limits}")
        } else {
            String::new()
        }
    }
}
