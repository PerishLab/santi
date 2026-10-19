use super::{Value, WebhookError, string_at};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Relation {
    Assignment,
    Release,
    Mention,
    Responsibility,
    Unrelated,
}

impl Relation {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Assignment => "assignment",
            Self::Release => "assignment_removed",
            Self::Mention => "mention",
            Self::Responsibility => "assigned_issue",
            Self::Unrelated => "unrelated",
        }
    }
}

pub(super) fn related(
    payload: &Value,
    event: &str,
    recipient: &str,
) -> Result<Relation, WebhookError> {
    let action = payload["action"].as_str().unwrap_or("");
    let changed = string_at(payload, &["assignee", "login"])
        .is_some_and(|login| login.eq_ignore_ascii_case(recipient));
    if event == "issues" && changed {
        match action {
            "assigned" => return Ok(Relation::Assignment),
            "unassigned" => return Ok(Relation::Release),
            _ => {}
        }
    }
    let direct = match (event, action) {
        ("issues", "opened") | ("issue_comment", "created" | "edited" | "deleted") => true,
        ("issues", "edited") => payload.pointer("/changes/body").is_some(),
        _ => false,
    };
    let source = if event == "issue_comment" {
        "comment"
    } else {
        "issue"
    };
    let current = string_at(payload, &[source, "body"]);
    let previous = string_at(payload, &["changes", "body", "from"]);
    let mentions = [current, previous]
        .into_iter()
        .flatten()
        .any(|body| mentioned(&body, recipient));
    if direct && mentions {
        return Ok(Relation::Mention);
    }
    let assigned = match payload.pointer("/issue/assignees") {
        Some(Value::Array(assignees)) => assignees.iter().any(|assignee| {
            assignee["login"]
                .as_str()
                .is_some_and(|login| login.eq_ignore_ascii_case(recipient))
        }),
        _ => match payload.pointer("/issue/assignee") {
            Some(Value::Null) => false,
            Some(assignee) => assignee["login"]
                .as_str()
                .is_some_and(|login| login.eq_ignore_ascii_case(recipient)),
            None => return Err(WebhookError::BadRequest(
                "GitHub issue assignment data is unavailable; supply the complete event payload"
                    .into(),
            )),
        },
    };
    Ok(if assigned {
        Relation::Responsibility
    } else {
        Relation::Unrelated
    })
}

pub(super) fn topic(repo: &str, number: i64) -> bool {
    !repo.is_empty() && repo.len() <= 256 && number > 0
}

pub(super) fn mentioned(body: &str, recipient: &str) -> bool {
    let needle = format!("@{}", recipient.to_ascii_lowercase());
    let mut fence = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"]
            .into_iter()
            .find(|marker| trimmed.starts_with(marker));
        if let Some(marker) = marker {
            fence = if fence == Some(marker) {
                None
            } else {
                fence.or(Some(marker))
            };
            continue;
        }
        if fence.is_some() {
            continue;
        }
        for text in prose(line) {
            let text = text.to_ascii_lowercase();
            if text.match_indices(&needle).any(|(start, _)| {
                let before = text[..start].chars().next_back();
                let after = text[start + needle.len()..].chars().next();
                before.is_none_or(|c| !c.is_ascii_alphanumeric() && !"_-.@\\".contains(c))
                    && after.is_none_or(|c| !c.is_ascii_alphanumeric() && !"_-".contains(c))
            }) {
                return true;
            }
        }
    }
    false
}

fn prose(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find('`') {
        let start = cursor + offset;
        let width = line[start..]
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        let after = start + width;
        let marker = "`".repeat(width);
        let end = line[after..]
            .match_indices(&marker)
            .find_map(|(offset, _)| {
                let before = line[..after + offset].chars().next_back();
                let next = line[after + offset + width..].chars().next();
                (before != Some('`') && next != Some('`')).then_some(after + offset)
            });
        let Some(end) = end else {
            break;
        };
        parts.push(&line[cursor..start]);
        cursor = end + width;
    }
    parts.push(&line[cursor..]);
    parts
}
