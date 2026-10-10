use super::*;

#[derive(Clone, Default)]
pub(super) struct BudgetedProvider {
    pub(super) requests: Arc<Mutex<Vec<Request>>>,
    pub(super) rounds: usize,
    pub(super) command: Option<String>,
    pub(super) maintenance: bool,
    pub(super) second: Option<String>,
    pub(super) batch: usize,
}

#[async_trait]
impl Provider for BudgetedProvider {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("output-provider"),
            model: "output-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let round = {
            let mut requests = self.requests.lock().unwrap();
            let round = requests.len();
            requests.push(request);
            round
        };
        if round < self.rounds {
            let command = self
                .command
                .clone()
                .unwrap_or_else(|| format!("printf '{}'", "x".repeat(100)));
            let compact = self.maintenance && maintenance(&self.requests, round);
            let arguments = if compact {
                json!({"summary": "The caller's work continues; completed tool results remain in history."})
            } else {
                json!({"command": command})
            };
            let count = if compact { 1 } else { self.batch.max(1) };
            let mut events = vec![Ok(Event::Text(format!("round {round}")))];
            for index in 0..count {
                events.push(Ok(Event::Called(Call {
                    response: format!("response_{round}"),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: format!("call_{round}_{index}"),
                    name: if compact {
                        "compact".into()
                    } else if round == 1 {
                        self.second.clone().unwrap_or_else(|| "shell".into())
                    } else {
                        "shell".into()
                    },
                    raw: arguments.to_string(),
                    arguments: arguments.clone(),
                })));
            }
            events.push(Ok(Event::Completed {
                response: Some(format!("response_{round}")),
            }));
            return Ok(Box::pin(stream::iter(events)));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("bounded output".to_string())),
            Ok(Event::Completed {
                response: Some("response_done".to_string()),
            }),
        ])))
    }
}

fn maintenance(requests: &Mutex<Vec<Request>>, round: usize) -> bool {
    requests.lock().unwrap()[round]
        .tools
        .as_ref()
        .is_some_and(|tools| {
            tools.len() == 1
                && serde_json::to_value(&tools[0]).unwrap()["Function"]["name"] == "compact"
        })
}
