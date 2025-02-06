use httpmock::{prelude::*, Mock};
use serde_json::json;
use soroban_rpc::{GetEventsResponse, GetNetworkResponse};
use soroban_test::{TestEnv, LOCAL_NETWORK_PASSPHRASE};

#[tokio::test]
async fn test_use_rpc_provider_with_auth_header() {
    // mock out http request to rpc provider
    let server = MockServer::start();
    let generate_account_mock = mock_generate_account(&server);
    let get_network_mock = mock_get_network(&server);
    let get_events_mock = mock_get_events(&server);

    // create a new test environment with the mock server
    let rpc_url = server.url("");
    let rpc_headers = vec![("Authorization".to_string(), "Bearer test-token".to_string())];
    let sandbox = &TestEnv::with_rpc_provider(&rpc_url, rpc_headers);

    sandbox
        .new_assert_cmd("events")
        .arg("--start-ledger")
        .arg("1000")
        .assert()
        .success();

    // generate account is being called in `with_rpc_provider`
    generate_account_mock.assert();
    // get_network and get_events are being called in the `stellar events` command
    get_network_mock.assert();
    get_events_mock.assert();
}

fn mock_generate_account(server: &MockServer) -> Mock {
    let cli_version = soroban_cli::commands::version::pkg();
    let agent = format!("soroban-cli/{cli_version}");
    server.mock(|when, then| {
        when.method(GET)
            .path("/friendbot")
            .header("accept", "*/*")
            .header("user-agent", agent);
        then.status(200);
    })
}

fn mock_get_network(server: &MockServer) -> Mock {
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Bearer test-token")
            .json_body(json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "getNetwork"
            }));

        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": GetNetworkResponse {
                friendbot_url: None,
                passphrase: LOCAL_NETWORK_PASSPHRASE.to_string(),
                protocol_version: 22}
        }));
    })
}

fn mock_get_events(server: &MockServer) -> Mock {
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Bearer test-token")
            .json_body(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getEvents",
                "params": {
                    "startLedger": 1000,
                    "filters": [
                        {
                            "contractIds": [],
                            "topics": []
                        }
                    ],
                    "pagination": {
                        "limit": 10
                    }
                }
            }));

        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": GetEventsResponse {
                events: vec![],
                latest_ledger: 1000
            }
        }));
    })
}
