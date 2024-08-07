// Copyright 2024 Golem Cloud
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::api_definition::{golem_def, make_golem_file, make_shopping_cart_component};
use crate::api_deployment::make_definition;
use crate::cli::{Cli, CliLive};
use crate::worker::make_component;
use golem_cli::model::component::ComponentView;
use golem_cli::model::WorkerMetadataView;
use golem_client::model::{ApiDeployment, HttpApiDefinition};
use golem_common::uri::oss::url::{ApiDefinitionUrl, ApiDeploymentUrl, ComponentUrl, WorkerUrl};
use golem_common::uri::oss::urn::{ApiDefinitionUrn, ApiDeploymentUrn, WorkerUrn};
use golem_test_framework::config::TestDependencies;
use libtest_mimic::{Failed, Trial};
use std::sync::Arc;

fn make(cli: CliLive, deps: Arc<dyn TestDependencies + Send + Sync + 'static>) -> Vec<Trial> {
    let ctx = (deps, cli);
    vec![
        Trial::test_in_context(
            "top_level_get_api_definition".to_string(),
            ctx.clone(),
            top_level_get_api_definition,
        ),
        Trial::test_in_context(
            "top_level_get_api_deployment".to_string(),
            ctx.clone(),
            top_level_get_api_deployment,
        ),
        Trial::test_in_context(
            "top_level_get_component".to_string(),
            ctx.clone(),
            top_level_get_component,
        ),
        Trial::test_in_context(
            "top_level_get_worker".to_string(),
            ctx.clone(),
            top_level_get_worker,
        ),
    ]
}

pub fn all(deps: Arc<dyn TestDependencies + Send + Sync + 'static>) -> Vec<Trial> {
    make(
        CliLive::make("top_level_get", deps.clone())
            .unwrap()
            .with_long_args(),
        deps,
    )
}

fn top_level_get_api_definition(
    (deps, cli): (Arc<dyn TestDependencies + Send + Sync + 'static>, CliLive),
) -> Result<(), Failed> {
    let component_name = "top_level_get_api_definition";
    let component = make_shopping_cart_component(deps, component_name, &cli)?;
    let component_id = component.component_urn.id.0.to_string();
    let def = golem_def(component_name, &component_id);
    let path = make_golem_file(&def)?;

    let _: HttpApiDefinition = cli.run(&["api-definition", "add", path.to_str().unwrap()])?;

    let url = ApiDefinitionUrl {
        name: component_name.to_string(),
        version: "0.1.0".to_string(),
    };

    let res: HttpApiDefinition = cli.run(&["get", &url.to_string()])?;

    assert_eq!(res, def);

    let urn = ApiDefinitionUrn {
        id: component_name.to_string(),
        version: "0.1.0".to_string(),
    };

    let res: HttpApiDefinition = cli.run(&["get", &urn.to_string()])?;

    assert_eq!(res, def);

    Ok(())
}

fn top_level_get_api_deployment(
    (deps, cli): (Arc<dyn TestDependencies + Send + Sync + 'static>, CliLive),
) -> Result<(), Failed> {
    let definition = make_definition(deps, &cli, "top_level_get_api_deployment")?;
    let host = "get-host-top-level-get";
    let cfg = &cli.config;

    let created: ApiDeployment = cli.run(&[
        "api-deployment",
        "deploy",
        &cfg.arg('d', "definition"),
        &format!("{}/{}", definition.id, definition.version),
        &cfg.arg('H', "host"),
        host,
        &cfg.arg('s', "subdomain"),
        "sdomain",
    ])?;

    let site = format!("sdomain.{host}");

    let url = ApiDeploymentUrl { site: site.clone() };

    let res: ApiDeployment = cli.run(&["get", &url.to_string()])?;

    assert_eq!(created, res);

    let urn = ApiDeploymentUrn { site: site.clone() };

    let res: ApiDeployment = cli.run(&["get", &urn.to_string()])?;

    assert_eq!(created, res);

    Ok(())
}

fn top_level_get_component(
    (deps, cli): (Arc<dyn TestDependencies + Send + Sync + 'static>, CliLive),
) -> Result<(), Failed> {
    let component_name = "top_level_get_component";
    let env_service = deps.component_directory().join("environment-service.wasm");
    let cfg = &cli.config;
    let component: ComponentView = cli.run(&[
        "component",
        "add",
        &cfg.arg('c', "component-name"),
        component_name,
        env_service.to_str().unwrap(),
    ])?;

    let url = ComponentUrl {
        name: component.component_name.to_string(),
    };

    let res: ComponentView = cli.run(&["get", &url.to_string()])?;
    assert_eq!(res, component);

    let res: ComponentView = cli.run(&["get", &component.component_urn.to_string()])?;
    assert_eq!(res, component);

    Ok(())
}

fn top_level_get_worker(
    (deps, cli): (Arc<dyn TestDependencies + Send + Sync + 'static>, CliLive),
) -> Result<(), Failed> {
    let component = make_component(deps, "top_level_get_worker", &cli)?;
    let worker_name = "top_level_get_worker";
    let cfg = &cli.config;

    let worker_urn: WorkerUrn = cli.run(&[
        "worker",
        "add",
        &cfg.arg('w', "worker-name"),
        worker_name,
        "--component",
        &component.component_urn.to_string(),
    ])?;

    let url = WorkerUrl {
        component_name: component.component_name.to_string(),
        worker_name: worker_name.to_string(),
    };

    let worker: WorkerMetadataView = cli.run(&["get", &url.to_string()])?;

    assert_eq!(worker.worker_urn, worker_urn);

    let worker: WorkerMetadataView = cli.run(&["get", &worker_urn.to_string()])?;

    assert_eq!(worker.worker_urn, worker_urn);

    Ok(())
}