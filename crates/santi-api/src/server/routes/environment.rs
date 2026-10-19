use super::*;
use santi_core::environ;

#[utoipa::path(
    get,
    path = "/api/v1/souls/{soul}/environment",
    params(("soul" = String, Path)),
    responses(
        (status = 200, body = [environ::Variable]),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn soul_environs() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(owner): Path<String>| async move {
            Environs(service).soul_environs(owner).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/souls/{soul}/environment",
    params(("soul" = String, Path)),
    request_body = environ::Draft,
    responses(
        (status = 200, body = environ::Variable),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn set_soul_environ() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(owner): Path<String>,
         Json(request): Json<environ::Draft>| async move {
            Environs(service).set_soul_environ(owner, request).await
        },
    )
}

#[utoipa::path(
    delete,
    path = "/api/v1/souls/{soul}/environment/{name}",
    params(("soul" = String, Path), ("name" = String, Path)),
    responses(
        (status = 204),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn end_soul_environ() -> MethodRouter<Service> {
    routing::delete(
        |State(service): State<Service>, Path((owner, name)): Path<(String, String)>| async move {
            Environs(service).end_soul_environ(owner, name).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/environment",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = [environ::Variable]),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn strand_environs() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(owner): Path<String>| async move {
            Environs(service).strand_environs(owner).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/environment",
    params(("strand" = String, Path)),
    request_body = environ::Draft,
    responses(
        (status = 200, body = environ::Variable),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn set_strand_environ() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(owner): Path<String>,
         Json(request): Json<environ::Draft>| async move {
            Environs(service).set_strand_environ(owner, request).await
        },
    )
}

#[utoipa::path(
    delete,
    path = "/api/v1/strands/{strand}/environment/{name}",
    params(("strand" = String, Path), ("name" = String, Path)),
    responses(
        (status = 204),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn end_strand_environ() -> MethodRouter<Service> {
    routing::delete(
        |State(service): State<Service>, Path((owner, name)): Path<(String, String)>| async move {
            Environs(service).end_strand_environ(owner, name).await
        },
    )
}

pub struct Environs(pub Service);

impl Environs {
    pub async fn soul_environs(
        &self,
        owner: String,
    ) -> Result<Json<Vec<environ::Variable>>, ApiError> {
        self.list(environ::Scope::Soul, owner).await
    }

    pub async fn set_soul_environ(
        &self,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, ApiError> {
        self.set(environ::Scope::Soul, owner, request).await
    }

    pub async fn end_soul_environ(
        &self,
        owner: String,
        name: String,
    ) -> Result<StatusCode, ApiError> {
        self.end(environ::Scope::Soul, owner, name).await
    }

    pub async fn strand_environs(
        &self,
        owner: String,
    ) -> Result<Json<Vec<environ::Variable>>, ApiError> {
        self.list(environ::Scope::Strand, owner).await
    }

    pub async fn set_strand_environ(
        &self,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, ApiError> {
        self.set(environ::Scope::Strand, owner, request).await
    }

    pub async fn end_strand_environ(
        &self,
        owner: String,
        name: String,
    ) -> Result<StatusCode, ApiError> {
        self.end(environ::Scope::Strand, owner, name).await
    }

    async fn list(
        &self,
        scope: environ::Scope,
        owner: String,
    ) -> Result<Json<Vec<environ::Variable>>, ApiError> {
        self.0
            .environs(scope, &owner)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn set(
        &self,
        scope: environ::Scope,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, ApiError> {
        self.0
            .set_environ(scope, &owner, request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn end(
        &self,
        scope: environ::Scope,
        owner: String,
        name: String,
    ) -> Result<StatusCode, ApiError> {
        self.0
            .end_environ(scope, &owner, &name)
            .await
            .map(|_| StatusCode::NO_CONTENT)
            .map_err(ApiError::from_service)
    }
}
