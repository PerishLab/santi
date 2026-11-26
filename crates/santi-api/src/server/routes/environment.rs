use super::*;
use santi_core::environ;

#[utoipa::path(
    get,
    path = "/api/v1/souls/{soul}/environment",
    operation_id = "soul_environs",
    params(("soul" = String, Path)),
    responses(
        (status = 200, body = [environ::Variable]),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn owned() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(owner): Path<String>| async move {
            Environs(service).owned(owner).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/souls/{soul}/environment",
    operation_id = "set_soul_environ",
    params(("soul" = String, Path)),
    request_body = environ::Draft,
    responses(
        (status = 200, body = environ::Variable),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn declare() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(owner): Path<String>,
         Json(request): Json<environ::Draft>| async move {
            Environs(service).declare(owner, request).await
        },
    )
}

#[utoipa::path(
    delete,
    path = "/api/v1/souls/{soul}/environment/{name}",
    operation_id = "end_soul_environ",
    params(("soul" = String, Path), ("name" = String, Path)),
    responses(
        (status = 204),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn revoke() -> MethodRouter<Service> {
    routing::delete(
        |State(service): State<Service>, Path((owner, name)): Path<(String, String)>| async move {
            Environs(service).revoke(owner, name).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/environment",
    operation_id = "strand_environs",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = [environ::Variable]),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn scoped() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(owner): Path<String>| async move {
            Environs(service).scoped(owner).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/environment",
    operation_id = "set_strand_environ",
    params(("strand" = String, Path)),
    request_body = environ::Draft,
    responses(
        (status = 200, body = environ::Variable),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn assign() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(owner): Path<String>,
         Json(request): Json<environ::Draft>| async move {
            Environs(service).assign(owner, request).await
        },
    )
}

#[utoipa::path(
    delete,
    path = "/api/v1/strands/{strand}/environment/{name}",
    operation_id = "end_strand_environ",
    params(("strand" = String, Path), ("name" = String, Path)),
    responses(
        (status = 204),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn unassign() -> MethodRouter<Service> {
    routing::delete(
        |State(service): State<Service>, Path((owner, name)): Path<(String, String)>| async move {
            Environs(service).unassign(owner, name).await
        },
    )
}

pub struct Environs(pub Service);

impl Environs {
    pub async fn owned(&self, owner: String) -> Result<Json<Vec<environ::Variable>>, Problem> {
        self.list(environ::Scope::Soul, owner).await
    }

    pub async fn declare(
        &self,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, Problem> {
        self.set(environ::Scope::Soul, owner, request).await
    }

    pub async fn revoke(&self, owner: String, name: String) -> Result<StatusCode, Problem> {
        self.end(environ::Scope::Soul, owner, name).await
    }

    pub async fn scoped(&self, owner: String) -> Result<Json<Vec<environ::Variable>>, Problem> {
        self.list(environ::Scope::Strand, owner).await
    }

    pub async fn assign(
        &self,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, Problem> {
        self.set(environ::Scope::Strand, owner, request).await
    }

    pub async fn unassign(&self, owner: String, name: String) -> Result<StatusCode, Problem> {
        self.end(environ::Scope::Strand, owner, name).await
    }

    async fn list(
        &self,
        scope: environ::Scope,
        owner: String,
    ) -> Result<Json<Vec<environ::Variable>>, Problem> {
        self.0
            .environs(scope, &owner)
            .await
            .map(Json)
            .map_err(Problem::service)
    }

    async fn set(
        &self,
        scope: environ::Scope,
        owner: String,
        request: environ::Draft,
    ) -> Result<Json<environ::Variable>, Problem> {
        self.0
            .export(scope, &owner, request)
            .await
            .map(Json)
            .map_err(Problem::service)
    }

    async fn end(
        &self,
        scope: environ::Scope,
        owner: String,
        name: String,
    ) -> Result<StatusCode, Problem> {
        self.0
            .unset(scope, &owner, &name)
            .await
            .map(|_| StatusCode::NO_CONTENT)
            .map_err(Problem::service)
    }
}
