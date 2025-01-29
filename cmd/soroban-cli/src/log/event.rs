use stellar_strkey::Contract;
use tracing::{debug, span, Level};

use crate::{print::Print, xdr};
use xdr::WriteXdr;

pub fn all(events: &[xdr::DiagnosticEvent]) {
    for (i, event) in events.iter().enumerate() {
        let span = if let Some(event) = parse_type(event) {
            event.span()
        } else {
            span!(Level::TRACE, "diagnostic_event")
        };
        let _enter = span.enter();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        let json = serde_json::to_string(event).unwrap();
        debug!("{i}: {xdr} {json}");
    }
}

struct Event {
    contract: Contract,
    r#type: Type,
}

enum Type {
    Contract(Vec<xdr::ScVal>, xdr::ScVal),
    Log(xdr::ScVal),
}

impl Type {
    pub fn span(&self) -> tracing::Span {
        match self {
            Type::Contract(_, _) => span!(Level::DEBUG, "contract_event"),
            Type::Log(_) => span!(Level::DEBUG, "log_event"),
        }
    }
}

fn parse_type(event: &xdr::DiagnosticEvent) -> Option<Type> {
    match &event.event.body {
        xdr::ContractEventBody::V0(xdr::ContractEventV0 { topics, data })
            if topics.len() == 1
                && matches!(event.event.type_, xdr::ContractEventType::Diagnostic) =>
        {
            if topics[0] == xdr::ScVal::Symbol(str_to_sc_symbol("log")) {
                Some(Type::Log(data.clone()))
            } else {
                None
            }
        }
        xdr::ContractEventBody::V0(xdr::ContractEventV0 { topics, data })
            if matches!(event.event.type_, xdr::ContractEventType::Contract) =>
        {
            Some(Type::Contract(topics.to_vec(), data.clone()))
        }
        xdr::ContractEventBody::V0(_) => None,
    }
}

fn parse_event(event: &xdr::DiagnosticEvent) -> Option<Event> {
    let r#type = parse_type(event)?;
    let contract = event
        .event
        .contract_id
        .clone()
        .map(|hash| Contract(hash.0))?;
    Some(Event { contract, r#type })
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}

pub fn contract(events: &[xdr::DiagnosticEvent], print: &Print) {
    for Event { contract, r#type } in events.iter().filter_map(parse_event) {
        match r#type {
            Type::Contract(topics, value) => {
                let topics = serde_json::to_string(&topics).unwrap();
                let value = serde_json::to_string(&value).unwrap();
                print.eventln(format!("{contract} - Event: {topics} = {value}"));
            }
            Type::Log(value) => {
                let value = serde_json::to_string(&value).unwrap();
                print.logln(format!("{contract} - Log: {value}"));
            }
        }
    }
}
