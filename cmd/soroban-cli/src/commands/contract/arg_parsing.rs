use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::ffi::OsString;
use std::fmt::Debug;
use std::path::PathBuf;

use clap::value_parser;
use ed25519_dalek::SigningKey;
use heck::ToKebabCase;

use crate::xdr::{
    self, Hash, InvokeContractArgs, ScSpecEntry, ScSpecFunctionV0, ScSpecTypeDef, ScVal, ScVec,
};

use crate::commands::txn_result::TxnResult;

use crate::config::{
    self,
    sc_address::{self, UnresolvedScAddress},
};

use soroban_spec_tools::Spec;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg {
        arg: String,
        error: soroban_spec_tools::Error,
    },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult {
        result: ScVal,
        error: soroban_spec_tools::Error,
    },
    #[error("function {0} was not found in the contract")]
    FunctionNotFoundInContractSpec(String),
    #[error("function name {0} is too long")]
    FunctionNameTooLong(String),
    #[error("argument count ({current}) surpasses maximum allowed count ({maximum})")]
    MaxNumberOfArgumentsReached { current: usize, maximum: usize },
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    StrVal(#[from] soroban_spec_tools::Error),
    #[error("Missing argument {0}")]
    MissingArgument(String),
    #[error("")]
    MissingFileArg(PathBuf),
    #[error(transparent)]
    ScAddress(#[from] sc_address::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
}

fn running_cmd() -> String {
    let mut args: Vec<String> = env::args().collect();

    if let Some(pos) = args.iter().position(|arg| arg == "--") {
        args.truncate(pos);
    }

    format!("{} --", args.join(" "))
}

pub fn build_host_function_parameters(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
) -> Result<(String, Spec, InvokeContractArgs, Vec<SigningKey>), Error> {
    let spec = Spec(Some(spec_entries.to_vec()));

    let mut cmd = clap::Command::new(running_cmd())
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);

    for ScSpecFunctionV0 { name, .. } in spec.find_functions()? {
        cmd = cmd.subcommand(build_custom_cmd(&name.to_utf8_string_lossy(), &spec)?);
    }
    cmd.build();
    let long_help = cmd.render_long_help();

    // get_matches_from exits the process if `help`, `--help` or `-h`are passed in the slop
    // see clap documentation for more info: https://github.com/clap-rs/clap/blob/v4.1.8/src/builder/command.rs#L631
    let mut matches_ = cmd.get_matches_from(slop);
    let Some((function, matches_)) = &matches_.remove_subcommand() else {
        println!("{long_help}");
        std::process::exit(1);
    };

    let func = spec.find_function(function)?;
    // create parsed_args in same order as the inputs to func
    let mut signers: Vec<SigningKey> = vec![];
    let parsed_args = func
        .inputs
        .iter()
        .map(|i| {
            let name = i.name.to_utf8_string()?;
            if let Some(mut val) = matches_.get_raw(&name) {
                let mut s = val
                    .next()
                    .unwrap()
                    .to_string_lossy()
                    .trim_matches('"')
                    .to_string();
                if matches!(i.type_, ScSpecTypeDef::Address) {
                    let addr = resolve_address(&s, config)?;
                    let signer = resolve_signer(&s, config);
                    s = addr;
                    if let Some(signer) = signer {
                        signers.push(signer);
                    }
                }
                spec.from_string(&s, &i.type_)
                    .map_err(|error| Error::CannotParseArg { arg: name, error })
            } else if matches!(i.type_, ScSpecTypeDef::Option(_)) {
                Ok(ScVal::Void)
            } else if let Some(arg_path) = matches_.get_one::<PathBuf>(&fmt_arg_file_name(&name)) {
                if matches!(i.type_, ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_)) {
                    Ok(ScVal::try_from(
                        &std::fs::read(arg_path)
                            .map_err(|_| Error::MissingFileArg(arg_path.clone()))?,
                    )
                    .map_err(|()| Error::CannotParseArg {
                        arg: name.clone(),
                        error: soroban_spec_tools::Error::Unknown,
                    })?)
                } else {
                    let file_contents = std::fs::read_to_string(arg_path)
                        .map_err(|_| Error::MissingFileArg(arg_path.clone()))?;
                    tracing::debug!(
                        "file {arg_path:?}, has contents:\n{file_contents}\nAnd type {:#?}\n{}",
                        i.type_,
                        file_contents.len()
                    );
                    spec.from_string(&file_contents, &i.type_)
                        .map_err(|error| Error::CannotParseArg { arg: name, error })
                }
            } else {
                Err(Error::MissingArgument(name))
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;

    let contract_address_arg = xdr::ScAddress::Contract(Hash(contract_id.0));
    let function_symbol_arg = function
        .try_into()
        .map_err(|()| Error::FunctionNameTooLong(function.clone()))?;

    let final_args =
        parsed_args
            .clone()
            .try_into()
            .map_err(|_| Error::MaxNumberOfArgumentsReached {
                current: parsed_args.len(),
                maximum: ScVec::default().max_len(),
            })?;

    let invoke_args = InvokeContractArgs {
        contract_address: contract_address_arg,
        function_name: function_symbol_arg,
        args: final_args,
    };

    Ok((function.clone(), spec, invoke_args, signers))
}

fn build_custom_cmd(name: &str, spec: &Spec) -> Result<clap::Command, Error> {
    let func = spec
        .find_function(name)
        .map_err(|_| Error::FunctionNotFoundInContractSpec(name.to_string()))?;

    // Parse the function arguments
    let inputs_map = &func
        .inputs
        .iter()
        .map(|i| (i.name.to_utf8_string().unwrap(), i.type_.clone()))
        .collect::<HashMap<String, ScSpecTypeDef>>();
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let mut cmd = clap::Command::new(name)
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);
    let kebab_name = name.to_kebab_case();
    if kebab_name != name {
        cmd = cmd.alias(kebab_name);
    }
    let doc: &'static str = Box::leak(func.doc.to_utf8_string_lossy().into_boxed_str());
    let long_doc: &'static str = Box::leak(arg_file_help(doc).into_boxed_str());

    cmd = cmd.about(Some(doc)).long_about(long_doc);
    for (name, type_) in inputs_map {
        let mut arg = clap::Arg::new(name);
        let file_arg_name = fmt_arg_file_name(name);
        let mut file_arg = clap::Arg::new(&file_arg_name);
        arg = arg
            .long(name)
            .alias(name.to_kebab_case())
            .num_args(1)
            .value_parser(clap::builder::NonEmptyStringValueParser::new())
            .long_help(spec.doc(name, type_)?);

        file_arg = file_arg
            .long(&file_arg_name)
            .alias(file_arg_name.to_kebab_case())
            .num_args(1)
            .hide(true)
            .value_parser(value_parser!(PathBuf))
            .conflicts_with(name);

        if let Some(value_name) = spec.arg_value_name(type_, 0) {
            let value_name: &'static str = Box::leak(value_name.into_boxed_str());
            arg = arg.value_name(value_name);
        }

        // Set up special-case arg rules
        arg = match type_ {
            ScSpecTypeDef::Bool => arg
                .num_args(0..1)
                .default_missing_value("true")
                .default_value("false")
                .num_args(0..=1),
            ScSpecTypeDef::Option(_val) => arg.required(false),
            ScSpecTypeDef::I256 | ScSpecTypeDef::I128 | ScSpecTypeDef::I64 | ScSpecTypeDef::I32 => {
                arg.allow_hyphen_values(true)
            }
            _ => arg,
        };

        cmd = cmd.arg(arg);
        cmd = cmd.arg(file_arg);
    }
    Ok(cmd)
}

fn fmt_arg_file_name(name: &str) -> String {
    format!("{name}-file-path")
}

fn arg_file_help(docs: &str) -> String {
    format!(
        r#"{docs}
Usage Notes:
Each arg has a corresponding --<arg_name>-file-path which is a path to a file containing the corresponding JSON argument.
Note: The only types which aren't JSON are Bytes and BytesN, which are raw bytes"#
    )
}

pub fn output_to_string(
    spec: &Spec,
    res: &ScVal,
    function: &str,
) -> Result<TxnResult<String>, Error> {
    let mut res_str = String::new();
    if let Some(output) = spec.find_function(function)?.outputs.first() {
        res_str = spec
            .xdr_to_json(res, output)
            .map_err(|e| Error::CannotPrintResult {
                result: res.clone(),
                error: e,
            })?
            .to_string();
    }
    Ok(TxnResult::Res(res_str))
}

fn resolve_address(addr_or_alias: &str, config: &config::Args) -> Result<String, Error> {
    let sc_address: UnresolvedScAddress = addr_or_alias.parse().unwrap();
    let account = match sc_address {
        UnresolvedScAddress::Resolved(addr) => addr.to_string(),
        addr @ UnresolvedScAddress::Alias(_) => {
            let addr = addr.resolve(&config.locator, &config.get_network()?.network_passphrase)?;
            match addr {
                xdr::ScAddress::Account(account) => account.to_string(),
                contract @ xdr::ScAddress::Contract(_) => contract.to_string(),
            }
        }
    };
    Ok(account)
}

fn resolve_signer(addr_or_alias: &str, config: &config::Args) -> Option<SigningKey> {
    config
        .locator
        .read_key(addr_or_alias)
        .ok()?
        .private_key(None)
        .ok()
        .map(|pk| SigningKey::from_bytes(&pk.0))
}
