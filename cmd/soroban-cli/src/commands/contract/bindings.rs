pub mod json;
pub mod python;
pub mod rust;
pub mod typescript;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Json Bindings
    Json(json::Cmd),

    /// Generate Rust bindings
    Rust(rust::Cmd),

    /// Generate a TypeScript / JavaScript package
    Typescript(Box<typescript::Cmd>),

    /// Generate Python bindings
    Python(python::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] json::Error),

    #[error(transparent)]
    Rust(#[from] rust::Error),

    #[error(transparent)]
    Typescript(#[from] typescript::Error),

    #[error(transparent)]
    Python(#[from] python::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Json(json) => json.run()?,
            Cmd::Rust(rust) => rust.run()?,
            Cmd::Typescript(ts) => ts.run().await?,
            Cmd::Python(python) => python.run()?,
        }
        Ok(())
    }
}
