//! `Invitations`: invite, respond, history, over a real socket, comparing values.

mod common;

use common::{create, event_json, invitations_of, items_of, set_id, with_server};
use fs_interfaces::events_capnp::Response;
use fs_interfaces::rpc::Problem;

const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";

#[tokio::test(flavor = "current_thread")]
async fn invite_respond_and_history_tell_the_same_story() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let invitations = invitations_of(&root).await;
        let id = create(&items, "inv1", "Meeting", "2026-12-01T10:00:00").await;

        let mut invite = invitations.invite_request();
        set_id(invite.get().init_id(), id);
        let mut list = invite.get().init_invitees(3);
        for (i, (name, email)) in [
            ("Ben", "ben@example.org"),
            ("Eva", "eva@example.org"),
            ("Ben again", "BEN@example.org"),
        ]
        .iter()
        .enumerate()
        {
            let mut one = list.reborrow().get(u32::try_from(i).unwrap());
            one.set_name(name);
            one.set_email(email);
        }
        let answer = invite.send().promise.await.unwrap();
        let content = event_json(answer.get().unwrap().get_event().unwrap());
        let people = content["participants"].as_object().unwrap();
        assert_eq!(
            people.len(),
            2,
            "a repeated address is one invitee: {people:?}"
        );
        assert!(
            people
                .values()
                .all(|p| p["participationStatus"] == "needs-action")
        );

        let mut respond = invitations.respond_request();
        set_id(respond.get().init_id(), id);
        respond.get().set_who("ben@example.org");
        respond.get().set_response(Response::TentativeAccept);
        respond.get().set_comment("maybe");
        let answer = respond.send().promise.await.unwrap();
        let content = event_json(answer.get().unwrap().get_event().unwrap());
        let ben = content["participants"]
            .as_object()
            .unwrap()
            .values()
            .find(|p| p["email"] == "ben@example.org")
            .unwrap();
        assert_eq!(ben["participationStatus"], "tentative");
        assert_eq!(ben["participationComment"], "maybe");

        let mut history = invitations.history_request();
        set_id(history.get().init_id(), id);
        let answer = history.send().promise.await.unwrap();
        let entries = answer.get().unwrap().get_items().unwrap();
        let seen: Vec<(String, String)> = entries
            .iter()
            .map(|e| {
                assert!(e.get_at().unwrap().get_unix_millis() > 0);
                (
                    e.get_response().unwrap().to_string().unwrap(),
                    e.get_comment().unwrap().to_string().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("Invite".into(), String::new()),
                ("Invite".into(), String::new()),
                ("TentativeAccept".into(), "maybe".into())
            ]
        );
        assert_eq!(
            entries.get(2).get_who().unwrap().to_str().unwrap(),
            "ben@example.org"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_answer_from_a_stranger_or_for_a_cancelled_event_is_a_conflict() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let invitations = invitations_of(&root).await;
        let id = create(&items, "inv2", "Meeting", "2026-12-01T10:00:00").await;
        let mut respond = invitations.respond_request();
        set_id(respond.get().init_id(), id);
        respond.get().set_who("nobody@example.org");
        respond.get().set_response(Response::Accept);
        let error = respond.send().promise.await.err().unwrap();
        let problem = Problem::from_error(&error).unwrap();
        assert_eq!(
            (problem.problem_type.as_str(), problem.status),
            (CONFLICT, 409)
        );
    })
    .await;
}
