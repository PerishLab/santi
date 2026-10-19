use super::{Value, json, scope::Relation, string_at};

pub(super) struct Event<'a> {
    pub payload: &'a Value,
    pub event: &'a str,
    pub actor: &'a str,
    pub recipient: Option<&'a str>,
    pub relation: Relation,
}

pub(super) fn rendered(request: Event<'_>) -> (String, Value) {
    let payload = request.payload;
    let repo = string_at(payload, &["repository", "full_name"]).unwrap_or_default();
    let number = payload
        .pointer("/issue/number")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let title = string_at(payload, &["issue", "title"]).unwrap_or_default();
    let source = if request.event == "issue_comment" {
        "comment"
    } else {
        "issue"
    };
    let body = string_at(payload, &[source, "body"]);
    let available = body.is_some();
    let body = body.unwrap_or_default();
    let url = string_at(payload, &[source, "html_url"]).unwrap_or_default();
    let visible = clipped(&body, 8192);
    let mut text = format!(
        "[GitHub · {}#{number}]\n{}\n\nTitle: {}\nReference: {}\n\n{} body:\n{visible}",
        clipped(&repo, 256),
        statement(&request),
        clipped(&title, 512),
        clipped(&url, 2048),
        if source == "comment" {
            "Comment"
        } else {
            "Issue"
        },
    );
    if let Some(updated) = string_at(payload, &[source, "updated_at"]) {
        text.push_str(&format!("\n\nUpdated: {}", clipped(&updated, 64)));
    }
    if !available {
        text.push_str("\n[Body unavailable in this event; the reference identifies the source.]");
    } else if visible.len() < body.len() {
        text.push_str("\n[Body truncated; the full content is available at the reference.]");
    }
    if title.len() > 512 {
        text.push_str("\n[Title truncated.]");
    }
    let metadata = json!({
        "source": source,
        "updated": string_at(payload, &[source, "updated_at"]),
        "available": available,
        "bytes": body.len(),
        "visible_bytes": visible.len(),
        "truncated": visible.len() < body.len(),
        "title_truncated": title.len() > 512,
    });
    (text, metadata)
}

fn statement(request: &Event<'_>) -> String {
    let actor = if request.actor.is_empty() {
        "An unspecified actor".into()
    } else {
        format!("@{}", clipped(request.actor, 64))
    };
    let recipient = clipped(request.recipient.unwrap_or(""), 64);
    let action = request.payload["action"].as_str().unwrap_or("");
    match request.relation {
        Relation::Assignment => format!("{actor} assigned you (@{recipient}) to this Issue."),
        Relation::Release => {
            format!("{actor} removed you (@{recipient}) as an Assignee of this Issue.")
        }
        Relation::Mention => format!(
            "{actor} {} {} addressing you (@{recipient}).",
            verb(request.event, action),
            if request.event == "issue_comment" {
                "a comment"
            } else {
                "an Issue"
            }
        ),
        Relation::Responsibility => responsible(request, &actor, recipient),
        Relation::Unrelated => format!("{actor} reported a GitHub event for this Issue."),
    }
}

fn responsible(request: &Event<'_>, actor: &str, recipient: &str) -> String {
    let action = request.payload["action"].as_str().unwrap_or("");
    if request.event == "issue_comment" {
        return format!(
            "{actor} {} a comment on an Issue assigned to you (@{recipient}).",
            verb(request.event, action)
        );
    }
    match action {
        "assigned" | "unassigned" => {
            format!("{actor} changed assignees on an Issue assigned to you (@{recipient}).")
        }
        "labeled" | "unlabeled" => {
            format!("{actor} changed labels on an Issue assigned to you (@{recipient}).")
        }
        _ => format!(
            "{actor} {} an Issue assigned to you (@{recipient}).",
            verb(request.event, action)
        ),
    }
}

fn verb(event: &str, action: &str) -> &'static str {
    match (event, action) {
        ("issue_comment", "created") => "posted",
        (_, "opened") => "opened",
        (_, "edited") => "updated",
        (_, "deleted") => "deleted",
        (_, "closed") => "closed",
        (_, "reopened") => "reopened",
        (_, "assigned" | "unassigned") => "changed assignees through",
        (_, "labeled" | "unlabeled") => "changed labels through",
        _ => "reported",
    }
}

fn clipped(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
