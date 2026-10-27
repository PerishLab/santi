use crate::cli::{ClientDefaults, Material, Materials, Person, Reading, Soul};
use crate::client::{Http, urlencoding_encode};
use anyhow::Result;

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
        let id = if scope == "strands" {
            defaults.resolve_strand(args.id)?
        } else {
            defaults.require()?.to_string()
        };
        let url = format!("{base}/api/v1/{scope}/{id}/materials");
        match args.command {
            Some(Reading::Query {
                reference,
                available,
                blob,
                offset,
                limit,
            }) => {
                let mut query =
                    format!("{url}?available={available}&offset={offset}&limit={limit}");
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
}
