pub mod api_definition;
pub mod api_deployment;
mod security_scheme;
pub mod worker;
pub mod worker_connect;

use crate::api::worker::WorkerApi;
use crate::service::Services;
use golem_worker_service_base::api::CustomHttpRequestApi;
use golem_worker_service_base::api::HealthcheckApi;
use poem::endpoint::PrometheusExporter;
use poem::{get, EndpointExt, Route};
use poem_openapi::OpenApiService;
use prometheus::Registry;

pub type ApiServices = (
    WorkerApi,
    api_definition::RegisterApiDefinitionApi,
    api_deployment::ApiDeploymentApi,
    security_scheme::SecuritySchemeApi,
    HealthcheckApi,
);

pub fn combined_routes(prometheus_registry: Registry, services: &Services) -> Route {
    let api_service = make_open_api_service(services);

    let ui = api_service.swagger_ui();
    let spec = api_service.spec_endpoint_yaml();
    let metrics = PrometheusExporter::new(prometheus_registry.clone());

    let connect_services = worker_connect::ConnectService::new(services.worker_service.clone());

    Route::new()
        .nest("/", api_service)
        .nest("/docs", ui)
        .nest("/specs", spec)
        .nest("/metrics", metrics)
        .at(
            "/v1/components/:component_id/workers/:worker_name/connect",
            get(worker_connect::ws.data(connect_services)),
        )
}

pub fn custom_request_route(services: &Services) -> Route {
    let custom_request_executor = CustomHttpRequestApi::new(
        services.worker_to_http_service.clone(),
        services.http_definition_lookup_service.clone(),
        services.fileserver_binding_handler.clone(),
        services.http_handler_binding_handler.clone(),
        services.gateway_session_store.clone(),
    );

    Route::new().nest("/", custom_request_executor)
}

pub fn make_open_api_service(services: &Services) -> OpenApiService<ApiServices, ()> {
    OpenApiService::new(
        (
            worker::WorkerApi {
                component_service: services.component_service.clone(),
                worker_service: services.worker_service.clone(),
            },
            api_definition::RegisterApiDefinitionApi::new(services.definition_service.clone()),
            api_deployment::ApiDeploymentApi::new(services.deployment_service.clone()),
            security_scheme::SecuritySchemeApi::new(services.security_scheme_service.clone()),
            HealthcheckApi,
        ),
        "Golem API",
        "1.0",
    )
}
