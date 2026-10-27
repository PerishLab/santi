use crate::cli::{ClientDefaults, Material, Materials, Person, Reading, Soul};
use crate::client::{Http, urlencoding_encode};
use anyhow::{Context, Result};

pub(in crate::client) async fn soul(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    args: Soul,
) -> Result<()> {
    match args.command {
        Some(Person::Material(material)) => {
            Seat::new(http, base, defaults).run("souls", material).await
        }
        None => {
            let id = args
                .id
                .as_deref()
                .map(Ok)
                .unwrap_or_else(|| defaults.require())?;
            http.get(&format!("{base}/api/v1/souls/{id}")).await
        }
    }
}

pub(in crate::client) struct Seat<'a, 'h> {
    http: &'a Http<'h>,
    base: &'a str,
    defaults: &'a ClientDefaults,
}

impl<'a, 'h> Seat<'a, 'h> {
    pub(in crate::client) fn new(
        http: &'a Http<'h>,
        base: &'a str,
        defaults: &'a ClientDefaults,
    ) -> Self {
        Self {
            http,
            base,
            defaults,
        }
    }

    pub(in crate::client) async fn run(&self, scope: &str, args: Materials) -> Result<()> {
        let Self {
            http,
            base,
            defaults,
        } = self;
        let explicit = match &args.command {
            Some(
                Reading::Load { id, .. } | Reading::Drop { id, .. } | Reading::Query { id, .. },
            ) => id.clone(),
            None => args.id,
        };
        let id = if scope == "strands" {
            defaults.resolve_strand(explicit)?
        } else {
            explicit
                .map(Ok)
                .unwrap_or_else(|| defaults.require().map(str::to_string))?
        };
        let url = format!("{base}/api/v1/{scope}/{id}/materials");
        match args.command {
            Some(Reading::Load {
                reference,
                operation,
                ..
            }) => self.change(&url, "load", reference, operation).await,
            Some(Reading::Drop {
                reference,
                operation,
                ..
            }) => self.change(&url, "drop", reference, operation).await,
            Some(Reading::Query {
                reference,
                available,
                effective,
                operation,
                context,
                id: _,
                blob,
                offset,
                limit,
            }) => {
                let mut query = format!(
                    "{url}?available={available}&effective={effective}&offset={offset}&limit={limit}"
                );
                append(&mut query, "operation", operation);
                append(&mut query, "context", context);
                if let Some(reference) = reference {
                    query.push_str(&format!("&reference={}", urlencoding_encode(&reference)));
                }
                if let Some(blob) = blob {
                    query.push_str(&format!("&blob={}", urlencoding_encode(&blob)));
                }
                http.get(&query).await
            }
            None if scope == "strands" => {
                http.post(
                    &url,
                    Some(serde_json::json!({"kind":args.kind.unwrap_or(Material::Skill).wire()})),
                )
                .await
            }
            None => anyhow::bail!(
                "soul material requires an action; use query --available or query <reference>"
            ),
        }
    }
    async fn change(
        &self,
        url: &str,
        action: &str,
        reference: String,
        operation: Option<String>,
    ) -> Result<()> {
        let operation =
            operation.unwrap_or_else(|| format!("mo_{}", uuid::Uuid::new_v4().simple()));
        self.http.post(&format!("{url}/{action}"), Some(serde_json::json!({"reference": reference, "operation": operation}))).await
            .with_context(|| format!("material operation {operation}: if the response is uncertain, query --operation {operation} before another mutation"))
    }
}

fn append(query: &mut String, name: &str, value: Option<String>) {
    if let Some(value) = value {
        query.push_str(&format!("&{name}={}", urlencoding_encode(&value)));
    }
}
