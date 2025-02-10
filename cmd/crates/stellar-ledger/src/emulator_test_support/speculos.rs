use std::{borrow::Cow, collections::HashMap, path::PathBuf, str::FromStr};
use testcontainers::{
    core::{Mount, WaitFor},
    Image,
};

const NAME: &str = "docker.io/zondax/builder-zemu";
const TAG: &str = "speculos-3a3439f6b45eca7f56395673caaf434c202e7005";
const TEST_SEED_PHRASE: &str =
    "\"other base behind follow wet put glad muscle unlock sell income october\"";

#[allow(dead_code)]
static ENV: &Map = &Map(phf::phf_map! {
    "BOLOS_SDK"=> "/project/deps/nanos-secure-sdk",
    "BOLOS_ENV" => "/opt/bolos",
    "DISPLAY" => "host.docker.internal:0",
});
struct Map(phf::Map<&'static str, &'static str>);

#[allow(clippy::implicit_hasher)]
impl From<&Map> for HashMap<String, String> {
    fn from(Map(map): &Map) -> Self {
        map.into_iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct Speculos {
    env: HashMap<String, String>,
    volumes: Vec<Mount>,
    cmd: String,
}

const DEFAULT_APP_PATH: &str = "/project/app/bin";
impl Speculos {
    #[allow(dead_code)]
    pub fn new(ledger_device_model: String) -> Self {
        #[allow(unused_mut)]
        let apps_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("test_fixtures")
            .join("apps");
        let volumes = vec![Mount::bind_mount(
            apps_dir.to_str().unwrap(),
            DEFAULT_APP_PATH,
        )];
        let cmd = Self::get_cmd(ledger_device_model);
        Speculos {
            env: ENV.into(),
            volumes,
            cmd,
        }
    }

    fn get_cmd(ledger_device_model: String) -> String {
        let device_model: DeviceModel = ledger_device_model.parse().unwrap();
        let container_elf_path = format!("{DEFAULT_APP_PATH}/{}", device_model.as_file());
        format!(
            "/home/zondax/speculos/speculos.py --log-level speculos:DEBUG --color JADE_GREEN \
            --display headless \
            -s {TEST_SEED_PHRASE} \
            -m {device_model}  {container_elf_path}"
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DeviceModel {
    NanoS,
    NanoSP,
    NanoX,
}

impl FromStr for DeviceModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "nanos" => Ok(DeviceModel::NanoS),
            "nanosp" => Ok(DeviceModel::NanoSP),
            "nanox" => Ok(DeviceModel::NanoX),
            _ => Err(format!("Unsupported device model: {}", s)),
        }
    }
}

impl std::fmt::Display for DeviceModel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DeviceModel::NanoS => write!(f, "nanos"),
            DeviceModel::NanoSP => write!(f, "nanosp"),
            DeviceModel::NanoX => write!(f, "nanox"),
        }
    }
}

impl DeviceModel {
    pub fn as_file(&self) -> &str {
        match self {
            DeviceModel::NanoS => "stellarNanoSApp.elf",
            DeviceModel::NanoSP => "stellarNanoSPApp.elf",
            DeviceModel::NanoX => "stellarNanoXApp.elf",
        }
    }
}

impl Image for Speculos {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("HTTP proxy started...")]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env.clone().into_iter().collect::<Vec<_>>()
    }

    fn mounts(&self) -> impl IntoIterator<Item = &Mount> {
        self.volumes.iter()
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<std::borrow::Cow<'_, str>>> {
        vec![self.cmd.clone()].into_iter()
    }
}
