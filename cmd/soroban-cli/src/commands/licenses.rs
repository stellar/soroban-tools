use clap::Parser;
use std::fmt::Debug;

use license_fetcher::get_package_list_macro;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Display the license text.
    #[arg(long, short)]
    pub verbose: bool,
}

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        let package_list = get_package_list_macro!();

        package_list.iter().for_each(|pkg| {
            println!(
                "Name: {name}\nVersion: {version}\nLicense: {license}",
                name = pkg.name,
                version = pkg.version,
                license = pkg
                    .license_identifier
                    .clone()
                    .unwrap_or("Unknown".to_string()),
            );

            if let Some(repo) = pkg.repository.clone() {
                println!("Repo: {repo}");
            }

            if let Some(url) = pkg.homepage.clone() {
                println!("URL: {url}");
            }

            if self.verbose {
                if let Some(text) = pkg.license_text.clone() {
                    println!("{text}");
                    println!("———");
                }
            }

            println!();
        });
    }
}
