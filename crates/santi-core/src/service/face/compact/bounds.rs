use crate::compact;
use crate::service::Service;

impl Service {
    pub(in crate::service::face) async fn bounded2(
        &self,
        strand: &str,
        request: &compact::Exec,
    ) -> Result<(String, String), String> {
        let bounds = Bounds {
            first: trimmed(request.first.as_deref()),
            last: trimmed(request.last.as_deref()),
            from: request.from,
            to: request.to,
        };
        match bounds {
            Bounds {
                first: Some(from),
                last: Some(to),
                from: None,
                to: None,
            } => Ok((from.to_string(), to.to_string())),
            Bounds {
                first: None,
                last: None,
                from: Some(from),
                to: Some(to),
            } => {
                let from = self
                    .store
                    .seated(strand, from)
                    .await?
                    .ok_or_else(|| seatless("from", from))?;
                let to = self
                    .store
                    .seated(strand, to)
                    .await?
                    .ok_or_else(|| seatless("to", to))?;
                Ok((from, to))
            }
            Bounds {
                first: None,
                last: None,
                from: None,
                to: None,
            } if !request.absorb.is_empty() => self.merged(strand, &request.absorb).await,
            Bounds {
                first: None,
                last: None,
                from: None,
                to: None,
            } => self.settled(strand).await,
            _ => Err("compact requires either first/last as message ids or from/to as message sequences, and never a mixture of the two".to_string()),
        }
    }
}

struct Bounds<'a> {
    first: Option<&'a str>,
    last: Option<&'a str>,
    from: Option<i64>,
    to: Option<i64>,
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn seatless(label: &str, sequence: i64) -> String {
    format!(
        "compact {label} {sequence} is not a message sequence in this strand; from/to take message sequences, while tool and turn records carry sequences of their own and are not messages; read the strand's messages to pick a boundary, or pass first/last with message ids instead"
    )
}
