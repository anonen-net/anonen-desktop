pub fn fingerprint(
    model_id: &str,
    training_use: &str,
    retention_kind: &str,
    retention_days: Option<i64>,
    release_stage: &str,
) -> String {
    let days = retention_days
        .map(|d| d.to_string())
        .unwrap_or_else(|| "-".to_string());
    let stage = if release_stage == "preview" {
        "/preview"
    } else {
        ""
    };
    format!("{model_id}@{training_use}/{retention_kind}/{days}{stage}")
}

pub fn needs_disclosure(acknowledged: &[String], fingerprint: &str) -> bool {
    !acknowledged.iter().any(|a| a == fingerprint)
}

pub const MAX_ACKNOWLEDGED: usize = 60;

pub fn with_acknowledged(acknowledged: &[String], fingerprint: &str) -> Vec<String> {
    if acknowledged.iter().any(|a| a == fingerprint) {
        return acknowledged.to_vec();
    }
    let mut out = Vec::with_capacity(acknowledged.len() + 1);
    out.push(fingerprint.to_string());
    out.extend(acknowledged.iter().cloned());
    out.truncate(MAX_ACKNOWLEDGED);
    out
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, needs_disclosure, with_acknowledged, MAX_ACKNOWLEDGED};

    #[test]
    fn unseen_policy_is_disclosed() {
        assert!(needs_disclosure(&[], "a/one@no/none/-"));
        let seen = with_acknowledged(&[], "a/one@no/none/-");
        assert!(!needs_disclosure(&seen, "a/one@no/none/-"));
    }

    #[test]
    fn a_changed_policy_is_disclosed_again() {
        let seen = with_acknowledged(&[], &fingerprint("a/one", "no", "days", Some(30), "ga"));
        assert!(needs_disclosure(
            &seen,
            &fingerprint("a/one", "no", "unspecified", None, "ga")
        ));
    }

    #[test]
    fn the_same_mark_is_not_stored_twice() {
        let once = with_acknowledged(&[], "m");
        assert_eq!(with_acknowledged(&once, "m").len(), 1);
    }

    #[test]
    fn marks_are_capped() {
        let mut acked: Vec<String> = Vec::new();
        for i in 0..MAX_ACKNOWLEDGED + 5 {
            acked = with_acknowledged(&acked, &format!("m{i}"));
        }
        assert_eq!(acked.len(), MAX_ACKNOWLEDGED);

        assert!(needs_disclosure(&acked, "m0"));
    }

    #[test]
    fn only_a_preview_changes_the_mark() {
        assert_eq!(
            fingerprint(
                "azure/MAI-Transcribe-2",
                "no",
                "unspecified",
                None,
                "preview"
            ),
            "azure/MAI-Transcribe-2@no/unspecified/-/preview"
        );

        assert_eq!(
            fingerprint("x/y", "unknown", "unknown", None, "ga"),
            "x/y@unknown/unknown/-"
        );

        let seen = with_acknowledged(&[], &fingerprint("a/one", "no", "none", None, "preview"));
        assert!(needs_disclosure(
            &seen,
            &fingerprint("a/one", "no", "none", None, "ga")
        ));
    }

    #[test]
    fn days_are_part_of_the_mark() {
        assert_ne!(
            fingerprint("a/one", "no", "days", Some(30), "ga"),
            fingerprint("a/one", "no", "days", Some(7), "ga")
        );
        assert_ne!(
            fingerprint("a/one", "no", "days", Some(30), "ga"),
            fingerprint("b/two", "no", "days", Some(30), "ga")
        );
    }
}
