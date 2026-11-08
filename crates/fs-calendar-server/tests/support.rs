//! The provider's report: every capability of the contract, as a consumer reads it.

mod common;

use common::with_server;

#[tokio::test(flavor = "current_thread")]
async fn the_support_report_lists_every_capability_of_the_contract() {
    with_server(|root| async move {
        let report = root.support_report_request().send().promise.await.unwrap();
        let report = report.get().unwrap().get_report().unwrap();
        assert_eq!(
            report.get_provider().unwrap().to_str().unwrap(),
            "fs-calendar"
        );
        assert_eq!(
            report.get_contract_version().unwrap().to_str().unwrap(),
            "events 0.1 (draft)"
        );
        let mut ids: Vec<String> = report
            .get_entries()
            .unwrap()
            .iter()
            .inspect(|e| {
                assert!(
                    e.get_supported(),
                    "{}",
                    e.get_capability().unwrap().to_str().unwrap()
                );
            })
            .map(|e| e.get_capability().unwrap().to_string().unwrap())
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "event.create",
                "event.delete",
                "event.export",
                "event.import",
                "event.occurrences",
                "event.read",
                "event.subscribe",
                "event.update",
                "invite.answer",
                "invite.create",
                "invite.read"
            ]
        );
    })
    .await;
}
