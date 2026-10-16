use serde_json::{Value, json};

pub(super) fn reported(called: Option<&str>, replied: &str) -> Value {
    json!({
        "called_at": called,
        "replied_at": replied,
        "call_to_result_ms": interval(called, replied),
    })
}

fn interval(called: Option<&str>, replied: &str) -> Option<i64> {
    let called = called?.parse::<jiff::Timestamp>().ok()?;
    let replied = replied.parse::<jiff::Timestamp>().ok()?;
    if replied < called {
        return None;
    }
    i64::try_from(replied.duration_since(called).as_millis()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "2026-10-16T06:45:23.352Z";
    const END: &str = "2026-10-16T06:58:39.584Z";

    #[test]
    fn intervals() {
        assert_eq!(reported(Some(START), END)["call_to_result_ms"], 796232);
        assert_eq!(interval(Some(START), START), Some(0));
        assert_eq!(
            interval(Some("2026-10-16T07:45:23.352+01:00"), END),
            Some(796232)
        );
    }

    #[test]
    fn unknown() {
        for (called, replied) in [
            (None, END),
            (Some("unknown"), END),
            (Some(START), "unknown"),
            (Some(END), START),
            (Some("2026-10-16T06:45:23.3529Z"), START),
        ] {
            let facts = reported(called, replied);
            assert!(facts["call_to_result_ms"].is_null());
            assert_eq!(facts["called_at"], json!(called));
            assert_eq!(facts["replied_at"], replied);
        }
    }
}
