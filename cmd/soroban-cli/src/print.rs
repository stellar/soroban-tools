use std::fmt::Display;

use soroban_env_host::xdr::{Error as XdrError, Transaction};

use crate::{
    config::network::Network,
    utils::{explorer_url_for_transaction, transaction_hash},
};

pub struct Print {
    pub quiet: bool,
}

impl Print {
    pub fn new(quiet: bool) -> Print {
        Print { quiet }
    }

    pub fn print<T: Display + Sized>(&self, message: T) {
        if !self.quiet {
            eprint!("{message}");
        }
    }

    pub fn println<T: Display + Sized>(&self, message: T) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }

    pub fn clear_line(&self) {
        if cfg!(windows) {
            eprint!("\r");
        } else {
            eprint!("\r\x1b[2K");
        }
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn log_transaction(
        &self,
        tx: &Transaction,
        network: &Network,
        show_link: bool,
    ) -> Result<(), XdrError> {
        let tx_hash = transaction_hash(tx, &network.network_passphrase)?;
        let hash = hex::encode(tx_hash);

        self.infoln(format!("Transaction hash is {hash}").as_str());

        if show_link {
            if let Some(url) = explorer_url_for_transaction(network, &hash) {
                self.linkln(url);
            }
        }

        Ok(())
    }
}

macro_rules! create_print_functions {
    ($name:ident, $nameln:ident, $icon:expr) => {
        impl Print {
            #[allow(dead_code)]
            pub fn $name<T: Display + Sized>(&self, message: T) {
                if !self.quiet {
                    eprint!("{} {}", $icon, message);
                }
            }

            #[allow(dead_code)]
            pub fn $nameln<T: Display + Sized>(&self, message: T) {
                if !self.quiet {
                    eprintln!("{} {}", $icon, message);
                }
            }
        }
    };
}

create_print_functions!(bucket, bucketln, "🪣");
create_print_functions!(check, checkln, "✅");
create_print_functions!(error, errorln, "❌");
create_print_functions!(globe, globeln, "🌎");
create_print_functions!(info, infoln, "ℹ️");
create_print_functions!(link, linkln, "🔗");
create_print_functions!(save, saveln, "💾");
create_print_functions!(search, searchln, "🔎");
create_print_functions!(warn, warnln, "⚠️");
