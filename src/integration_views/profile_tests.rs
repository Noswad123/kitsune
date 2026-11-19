use super::*;
use crate::api::schema::IntegrationViewInformation;

fn item() -> IntegrationViewItem {
    IntegrationViewItem {
        id: "item".into(),
        label: "Owner name".into(),
        detail: "diagnostic fallback".into(),
        presentation: None,
        information: Some(IntegrationViewInformation::default()),
    }
}

#[test]
fn information_profiles_reject_atomically_without_renewing_or_clearing_intents() {
    for version in [1, 2, 3] {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new(vec!["owner".into()]);
        let opened = state
            .open(
                &ViewOpenParams {
                    source: "owner".into(),
                    version,
                },
                now,
            )
            .unwrap();
        let mut params = ViewReplaceParams {
            source: "owner".into(),
            lease: opened.lease,
            epoch: opened.epoch,
            revision: 1,
            items: vec![item()],
        };
        params.items[0].information = None;
        state.replace(&params, now).unwrap();
        assert!(state.activate("owner", &params.epoch, 1, "item", now));
        let deadline = state.owners["owner"].deadline;
        params.revision = 2;
        params.items.push(item());
        params.items[1].id = "second".into();
        if version == 3 {
            params.items[1].information.as_mut().unwrap().attention = vec![" ".into()];
        }
        assert_eq!(
            state.replace(&params, now + Duration::from_secs(1)),
            Err("integration_view_invalid_frame")
        );
        let owner = &state.owners["owner"];
        assert_eq!(owner.deadline, deadline);
        assert_eq!(owner.revision, 1);
        assert_eq!(owner.items.len(), 1);
        assert_eq!(owner.intents.len(), 1);
        params.items = vec![item()];
        if version == 3 {
            state.replace(&params, now).unwrap();
            assert!(state.project("owner", now).unwrap().items[0]
                .information
                .is_some());
        } else {
            assert!(state.replace(&params, now).is_err());
        }
    }
}

#[test]
fn information_utf8_bounds_bidi_and_source_quota() {
    let mut valid = item();
    let info = valid.information.as_mut().unwrap();
    info.attention = vec!["界".repeat(85); 4];
    info.activity = vec!["é".repeat(128); 2];
    assert!(validate_frame(&[valid.clone()]).is_ok());
    for bad in [
        "".into(),
        " \t".into(),
        "a\nb".into(),
        "a\u{202e}b".into(),
        "a\u{2066}b".into(),
        "界".repeat(86),
    ] {
        let mut invalid = item();
        invalid.information.as_mut().unwrap().context = vec![bad];
        assert!(validate_frame(&[invalid]).is_err());
    }
    let mut invalid = item();
    invalid.information.as_mut().unwrap().context = vec!["x".into(); 5];
    assert!(validate_frame(&[invalid]).is_err());
    let mut invalid = valid.clone();
    invalid.information.as_mut().unwrap().context = vec!["1234".into()];
    assert!(validate_frame(&[invalid.clone()]).is_ok());
    invalid.information.as_mut().unwrap().context = vec!["12345".into()];
    assert!(validate_frame(&[invalid]).is_err());
    let frames: Vec<_> = (0..128)
        .map(|n| {
            let mut next = valid.clone();
            next.id = n.to_string();
            next
        })
        .collect();
    assert!(validate_frame(&frames).is_err());
}
