use super::info::shared;
use crate::{
    commands::{contract::info::shared::fetch_wasm, global},
    print::Print,
    utils::http,
};
use base64::Engine as _;
use clap::{command, Parser};
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use std::fmt::Debug;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),
    #[error(transparent)]
    Spec(#[from] contract::Error),
    #[error("'source_repo' meta entry is not stored in the contract")]
    SourceRepoNotSpecified,
    #[error("'source_repo' meta entry '{0}' has prefix unsupported, only 'github:' supported")]
    SourceRepoUnsupported(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("GitHub attestation not found")]
    AttestationNotFound,
    #[error("GitHub attestation invalid")]
    AttestationInvalid,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        print.infoln("Loading wasm...");
        let Some(bytes) = fetch_wasm(&self.common).await? else {
            return Err(Error::SourceRepoNotSpecified);
        };
        let wasm_hash = Sha256::digest(&bytes);
        let wasm_hash_hex = hex::encode(wasm_hash);
        print.infoln(format!("Wasm Hash: {wasm_hash_hex}"));

        let spec = Spec::new(&bytes)?;
        let Some(source_repo) = spec.meta.iter().find_map(|meta_entry| {
            let ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) = meta_entry;
            if key.to_string() == "source_repo" {
                Some(val.to_string())
            } else {
                None
            }
        }) else {
            return Err(Error::SourceRepoNotSpecified);
        };
        print.infoln(format!("Source Repo: {source_repo}"));
        let Some(github_source_repo) = source_repo.strip_prefix("github:") else {
            return Err(Error::SourceRepoUnsupported(source_repo));
        };

        let url = format!(
            "https://api.github.com/repos/{github_source_repo}/attestations/sha256:{wasm_hash_hex}"
        );
        print.infoln(format!("Collecting GitHub attestation from {url}..."));
        let resp = http::client().get(url).send().await?;
        let resp: gh_attest_resp::Root = resp.json().await?;
        let Some(attestation) = resp.attestations.first() else {
            return Err(Error::AttestationNotFound);
        };
        let Ok(payload) = base64::engine::general_purpose::STANDARD
            .decode(&attestation.bundle.dsse_envelope.payload)
        else {
            return Err(Error::AttestationInvalid);
        };
        let payload: gh_payload::Root = serde_json::from_slice(&payload)?;
        print.checkln("Attestation found linked to GitHub Actions Workflow Run:");
        let workflow_repo = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .repository;
        let workflow_ref = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .ref_field;
        let workflow_path = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .path;
        let git_commit = &payload
            .predicate
            .build_definition
            .resolved_dependencies
            .first()
            .unwrap()
            .digest
            .git_commit;
        let runner_environment = payload
            .predicate
            .build_definition
            .internal_parameters
            .github
            .runner_environment
            .as_str();
        print.checkln(format!(" • Repository: {workflow_repo}"));
        print.checkln(format!(" • Ref:        {workflow_ref}"));
        print.checkln(format!(" • Path:       {workflow_path}"));
        print.checkln(format!(" • Git Commit: {git_commit}"));
        match runner_environment
        {
            runner @ "github-hosted" => print.checkln(format!(" • Runner:     {runner}")),
            runner => print.warnln(format!(" • Runner:     {runner} (runners not hosted by GitHub could have any configuration or environmental changes)")),
        }
        print.checkln(format!(
            " • Run:        {}",
            payload.predicate.run_details.metadata.invocation_id
        ));
        print.globeln(format!(
            "View the workflow at {workflow_repo}/blob/{git_commit}/{workflow_path}"
        ));
        print.globeln(format!(
            "View the repo at {workflow_repo}/tree/{git_commit}"
        ));

        Ok(())
    }
}

mod gh_attest_resp {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub attestations: Vec<Attestation>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attestation {
        pub bundle: Bundle,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Bundle {
        pub dsse_envelope: DsseEnvelope,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DsseEnvelope {
        pub payload: String,
    }
}

mod gh_payload {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub predicate_type: String,
        pub predicate: Predicate,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Predicate {
        pub build_definition: BuildDefinition,
        pub run_details: RunDetails,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BuildDefinition {
        pub external_parameters: ExternalParameters,
        pub internal_parameters: InternalParameters,
        pub resolved_dependencies: Vec<ResolvedDependency>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExternalParameters {
        pub workflow: Workflow,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Workflow {
        #[serde(rename = "ref")]
        pub ref_field: String,
        pub repository: String,
        pub path: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InternalParameters {
        pub github: Github,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Github {
        #[serde(rename = "runner_environment")]
        pub runner_environment: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResolvedDependency {
        pub uri: String,
        pub digest: Digest,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Digest {
        pub git_commit: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RunDetails {
        pub metadata: Metadata,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Metadata {
        pub invocation_id: String,
    }
}
