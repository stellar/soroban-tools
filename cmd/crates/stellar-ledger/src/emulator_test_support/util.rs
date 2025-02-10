use serde::Deserialize;
use std::ops::Range;
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::{Error, LedgerSigner};
use std::net::TcpListener;

use super::{http_transport::Emulator, speculos::Speculos};

use std::{collections::HashMap, time::Duration};

use stellar_xdr::curr::Hash;

use testcontainers::{core::ContainerPort, runners::AsyncRunner, ContainerAsync, ImageExt};
use tokio::time::sleep;

static PORT_RANGE: LazyLock<Mutex<Range<u16>>> = LazyLock::new(|| Mutex::new(40000..50000));

pub const TEST_NETWORK_PASSPHRASE: &[u8] = b"Test SDF Network ; September 2015";
pub fn test_network_hash() -> Hash {
    use sha2::Digest;
    Hash(sha2::Sha256::digest(TEST_NETWORK_PASSPHRASE).into())
}

pub async fn ledger(host_port: u16) -> LedgerSigner<Emulator> {
    LedgerSigner::new(get_http_transport("127.0.0.1", host_port).await.unwrap())
}

pub async fn click(ui_host_port: u16, url: &str) {
    let previous_events = get_emulator_events(ui_host_port).await;

    let client = reqwest::Client::new();
    let mut payload = HashMap::new();
    payload.insert("action", "press-and-release");

    let mut screen_has_changed = false;

    client
        .post(format!("http://localhost:{ui_host_port}/{url}"))
        .json(&payload)
        .send()
        .await
        .unwrap();

    while !screen_has_changed {
        let current_events = get_emulator_events(ui_host_port).await;

        if !(previous_events == current_events) {
            screen_has_changed = true
        }
    }

    sleep(Duration::from_secs(1)).await;
}

pub async fn enable_hash_signing(ui_host_port: u16) {
    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/both").await;

    click(ui_host_port, "button/both").await;

    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/both").await;
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct EmulatorEvent {
    text: String,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    events: Vec<EmulatorEvent>,
}

pub async fn get_container(ledger_device_model: &str) -> ContainerAsync<Speculos> {
    let (tcp_port_1, tcp_port_2) = get_available_ports(2);
    Speculos::new(ledger_device_model.to_string())
        .with_mapped_port(tcp_port_1, ContainerPort::Tcp(9998))
        .with_mapped_port(tcp_port_2, ContainerPort::Tcp(5000))
        .start()
        .await
        .unwrap()
}

pub fn get_available_ports(n: usize) -> (u16, u16) {
    let mut range = PORT_RANGE.lock().unwrap();
    let mut ports = Vec::with_capacity(n);
    while ports.len() < n {
        if let Some(port) = range.next() {
            if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
                ports.push(port);
                drop(listener);
            }
        } else {
            panic!("No more available ports");
        }
    }

    (ports[0], ports[1])
}

pub async fn get_http_transport(host: &str, port: u16) -> Result<Emulator, Error> {
    let max_retries = 5;
    let mut retries = 0;
    let mut wait_time = Duration::from_secs(1);
    // ping the emulator port to make sure it's up and running
    // retry with exponential backoff
    loop {
        match reqwest::get(format!("http://{host}:{port}")).await {
            Ok(_) => return Ok(Emulator::new(host, port)),
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    println!("get_http_transport: Exceeded max retries for connecting to emulated device");

                    return Err(Error::APDUExchangeError(format!(
                        "Failed to connect to emulator: {e}"
                    )));
                }
                sleep(wait_time).await;
                wait_time *= 2;
            }
        }
    }
}

pub async fn wait_for_emulator_start_text(ui_host_port: u16) {
    let mut ready = false;
    while !ready {
        let events = get_emulator_events_with_retries(ui_host_port, 5).await;

        if events.iter().any(|event| event.text == "is ready") {
            ready = true;
        }
    }
}

pub async fn wait_for_review_transaction_text(ui_host_port: u16) {
    let mut review_ready = false;
    while !review_ready {
        let events = get_emulator_events_with_retries(ui_host_port, 5).await;

        if events.iter().any(|event| event.text == "Review") {
            review_ready = true;
        }
    }
}

pub async fn get_emulator_events(ui_host_port: u16) -> Vec<EmulatorEvent> {
    // Allowing for less retries here because presumably the emulator should be up and running since we waited
    // for the "is ready" text via wait_for_emulator_start_text
    get_emulator_events_with_retries(ui_host_port, 1).await
}

pub async fn get_emulator_events_with_retries(
    ui_host_port: u16,
    max_retries: u16,
) -> Vec<EmulatorEvent> {
    let client = reqwest::Client::new();
    let mut retries = 0;
    let mut wait_time = Duration::from_secs(1);
    loop {
        match client
            .get(format!("http://localhost:{ui_host_port}/events"))
            .send()
            .await
        {
            Ok(req) => {
                let resp = req.json::<EventsResponse>().await.unwrap();
                return resp.events;
            }
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    println!("get_emulator_events_with_retries: Exceeded max retries");
                    panic!("get_emulator_events_with_retries: Failed to get emulator events: {e}");
                }
                sleep(wait_time).await;
                wait_time *= 2;
            }
        }
    }
}

pub async fn approve_tx_hash_signature(ui_host_port: u16, device_model: String) {
    wait_for_review_transaction_text(ui_host_port).await;
    let number_of_right_clicks = if device_model == "nanos" { 10 } else { 6 };
    for _ in 0..number_of_right_clicks {
        click(ui_host_port, "button/right").await;
    }

    click(ui_host_port, "button/both").await;
}

pub async fn approve_tx_signature(ui_host_port: u16, device_model: String) {
    let number_of_right_clicks = if device_model == "nanos" { 17 } else { 11 };
    for _ in 0..number_of_right_clicks {
        click(ui_host_port, "button/right").await;
    }
    click(ui_host_port, "button/both").await;
}
