use utoipa::Modify;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};

pub(super) struct Security;

impl Modify for Security {
    fn modify(&self, document: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = document.components.as_mut() {
            components.add_security_scheme(
                "catalog_operator",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("x-santi-catalog-token"))),
            );
            components.add_security_scheme(
                "downstream_bearer",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}
