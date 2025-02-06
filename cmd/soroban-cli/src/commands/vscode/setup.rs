use super::shared::{Args, Error};

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    args: Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let mut settings = self.args.read_settings()?;
        if settings.add_schema(self.args.settings_dir()?.as_path())? {
            self.args.write_settings(&settings)?;
            self.args.write_schema_file()?;
        }
        Ok(())
    }
}
