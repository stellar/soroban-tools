use stellar_xdr::{
    cli::{
        encode::{InputFormat, OutputFormat},
        Channel,
    },
    curr::TypeVariant,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] stellar_xdr::cli::encode::Error),
}

/// Encode a transaction envelope from JSON to XDR
#[derive(Debug, clap::Parser, Clone, Default)]
pub struct Cmd;

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let cmd = stellar_xdr::cli::encode::Cmd {
            files: vec![],
            r#type: TypeVariant::TransactionEnvelope.to_string(),
            input: InputFormat::Json,
            output: OutputFormat::SingleBase64,
        };
        cmd.run(&Channel::Curr)?;
        Ok(())
    }
}
