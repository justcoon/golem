// Copyright 2024-2025 Golem Cloud
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

use std::fmt::Display;
use std::sync::Arc;

use crate::gateway_api_definition::http::CompiledHttpApiDefinition;
use crate::gateway_api_deployment::ApiSiteString;
use crate::service::gateway::api_deployment::ApiDeploymentService;
use async_trait::async_trait;
use golem_common::model::HasAccountId;
use tracing::error;

// To lookup the set of API Definitions based on an incoming input.
// The input can be HttpRequest or GrpcRequest and so forth, and ApiDefinition
// depends on what is the input. There cannot be multiple types of ApiDefinition
// for a given input type.
#[async_trait]
pub trait HttpApiDefinitionsLookup<Namespace> {
    async fn get(
        &self,
        host: &ApiSiteString,
    ) -> Result<Vec<CompiledHttpApiDefinition<Namespace>>, ApiDefinitionLookupError>;
}

pub struct ApiDefinitionLookupError(pub String);

impl Display for ApiDefinitionLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApiDefinitionLookupError: {}", self.0)
    }
}

pub struct DefaultHttpApiDefinitionLookup<AuthCtx, Namespace> {
    deployment_service: Arc<dyn ApiDeploymentService<AuthCtx, Namespace> + Sync + Send>,
}

impl<AuthCtx, Namespace> DefaultHttpApiDefinitionLookup<AuthCtx, Namespace> {
    pub fn new(
        deployment_service: Arc<dyn ApiDeploymentService<AuthCtx, Namespace> + Sync + Send>,
    ) -> Self {
        Self { deployment_service }
    }
}

#[async_trait]
impl<AuthCtx, Namespace: HasAccountId + Send + Sync> HttpApiDefinitionsLookup<Namespace>
    for DefaultHttpApiDefinitionLookup<AuthCtx, Namespace>
{
    async fn get(
        &self,
        host: &ApiSiteString,
    ) -> Result<Vec<CompiledHttpApiDefinition<Namespace>>, ApiDefinitionLookupError> {
        let http_api_defs = self
            .deployment_service
            .get_all_definitions_by_site(host)
            .await
            .map_err(|err| {
                error!("Error getting API definitions from the repo: {}", err);
                ApiDefinitionLookupError(format!(
                    "Error getting API definitions from the repo: {}",
                    err
                ))
            })?;

        if http_api_defs.is_empty() {
            return Err(ApiDefinitionLookupError(format!(
                "API deployment with site: {} not found",
                &host
            )));
        }

        Ok(http_api_defs)
    }
}
