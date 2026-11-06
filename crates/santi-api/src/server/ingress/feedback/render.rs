use santi_core::receipt::{State, Status};
use serde_json::{Value, json};

pub(super) const SOURCE: &str = "https://github.com/PerishLab/santi#feishu-feedback-v1";

pub(super) fn document(text: &str, automatic: bool) -> Value {
    let mut line = Vec::new();
    if automatic {
        line.push(json!({"tag":"a", "text":"◌", "href":SOURCE}));
    }
    line.push(
        json!({"tag":"text", "text": if automatic { format!(" {text}") } else { text.into() }}),
    );
    json!({"content":[line]})
}

pub(super) fn automatic(status: &Status) -> Value {
    let text = match status.state {
        State::Accepted | State::Recovered => "等待处理",
        State::Driving => "正在处理",
        State::Failed => "处理已中断",
        State::Completed => "本轮已结束",
    };
    document(text, true)
}
