use soroban_cli::xdr::{self, ReadXdr};
use std::sync::{Arc, Mutex};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};

struct TestSubscriber {
    logs: Arc<Mutex<Vec<String>>>,
}

impl<S: Subscriber> tracing_subscriber::Layer<S> for TestSubscriber {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Capture the event data
        let mut logs = self.logs.lock().unwrap();
        logs.push(format!("{event:?}"));
    }
}

#[test]
fn test_diagnostic_events_logging() {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let subscriber = TestSubscriber { logs: logs.clone() };

    tracing::subscriber::with_default(tracing_subscriber::registry().with(subscriber), || {
        let events = [
                "AAAAAAAAAAAAAAAAAAAAAgAAAAAAAAADAAAADwAAAAdmbl9jYWxsAAAAAA0AAAAgfKvD/pIJPlRnGd3RKaBZSHfoq/nJbJSYxkVTScSbhuYAAAAPAAAABGRlY3IAAAAB",
                "AAAAAAAAAAAAAAABfKvD/pIJPlRnGd3RKaBZSHfoq/nJbJSYxkVTScSbhuYAAAACAAAAAAAAAAEAAAAPAAAAA2xvZwAAAAAQAAAAAQAAAAIAAAAOAAAACWNvdW50OiB7fQAAAAAAAAMAAAAA",
                "AAAAAAAAAAAAAAABfKvD/pIJPlRnGd3RKaBZSHfoq/nJbJSYxkVTScSbhuYAAAACAAAAAAAAAAIAAAAPAAAABWVycm9yAAAAAAAAAgAAAAEAAAAGAAAAEAAAAAEAAAACAAAADgAAACdWTSBjYWxsIHRyYXBwZWQ6IFVucmVhY2hhYmxlQ29kZVJlYWNoZWQAAAAADwAAAARkZWNy",
            ].iter().map(|event| xdr::DiagnosticEvent::from_xdr_base64(event,xdr::Limits::none()).unwrap()).collect::<Vec<_>>();
        soroban_cli::log::event::all(&events);
    });

    let captured_logs = logs.lock().unwrap();
    assert!(captured_logs.iter().any(|log| log.contains(r#"AAAAAAAAAAAAAAAAAAAAAgAAAAAAAAADAAAADwAAAAdmbl9jYWxsAAAAAA0AAAAgfKvD/pIJPlRnGd3RKaBZSHfoq/nJbJSYxkVTScSbhuYAAAAPAAAABGRlY3IAAAAB {"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"7cabc3fe92093e546719ddd129a0594877e8abf9c96c9498c6455349c49b86e6"},{"symbol":"decr"}],"data":"void"}}}}"#)));
    assert!(captured_logs
        .iter()
        .any(|log| log.contains("VM call trapped")));
}
