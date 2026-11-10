//! `Events.types` over a real socket: the catalog holds the `Event` description, equal to the core's.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

mod common;

use common::with_server;
use fs_calendar_core::Event;
use fs_types::{DescribesType, TypeRegistry};

#[tokio::test(flavor = "current_thread")]
async fn types_answers_the_event_description() {
    with_server(|root| async move {
        let catalog = root
            .types_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_catalog()
            .unwrap();
        let response = catalog.describe_request().send().promise.await.unwrap();
        let registry = TypeRegistry::read(response.get().unwrap().get_types().unwrap()).unwrap();
        assert_eq!(registry.len(), 1);
        let event = registry.get("Event").expect("Event is described");
        assert_eq!(event, &Event::describe());
        assert_eq!(event.program, "fs-calendar");
        // supported travels over the wire
        let find = |n: &str| event.find_command(n).unwrap().supported;
        assert!(find("create") && find("change") && find("find") && find("agenda"));
        assert!(!find("quickEntry") && !find("bookingPage") && !find("suggest"));
        assert!(event.record_commands().iter().any(|c| c.name == "change"));
    })
    .await;
}
