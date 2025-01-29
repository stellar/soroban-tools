#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: super::new::manage_data::Cmd,
}
