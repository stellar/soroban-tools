use clap::ValueEnum;
use stellar_xdr::{
    cli::{decode::InputFormat, Channel},
    curr::TypeVariant,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] stellar_xdr::cli::decode::Error),
}

/// Decode a transaction envelope from XDR to JSON
#[derive(Debug, clap::Parser, Clone, Default)]
pub struct Cmd {
    // Output format
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(Default, Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Json,
    JsonFormatted,
}

impl From<OutputFormat> for stellar_xdr::cli::decode::OutputFormat {
    fn from(v: OutputFormat) -> Self {
        match v {
            OutputFormat::Json => Self::Json,
            OutputFormat::JsonFormatted => Self::JsonFormatted,
        }
    }
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let cmd = stellar_xdr::cli::decode::Cmd {
            files: vec![],
            r#type: TypeVariant::TransactionEnvelope.to_string(),
            input: InputFormat::SingleBase64,
            output: self.output.into(),
        };
        cmd.run(&Channel::Curr)?;
        Ok(())
    }
}
