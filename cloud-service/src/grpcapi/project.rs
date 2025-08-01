// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::auth::AccountAuthorisation;
use crate::grpcapi::get_authorisation_token;
use crate::model;
use crate::service::auth::{AuthService, AuthServiceError};
use crate::service::project;
use golem_api_grpc::proto::golem::common::{Empty, ErrorBody, ErrorsBody};
use golem_api_grpc::proto::golem::project::v1::cloud_project_service_server::CloudProjectService;
use golem_api_grpc::proto::golem::project::v1::{
    create_project_response, delete_project_response, get_default_project_response,
    get_project_response, get_projects_response, project_error, CreateProjectRequest,
    CreateProjectResponse, CreateProjectSuccessResponse, DeleteProjectRequest,
    DeleteProjectResponse, GetDefaultProjectRequest, GetDefaultProjectResponse, GetProjectRequest,
    GetProjectResponse, GetProjectsRequest, GetProjectsResponse, GetProjectsSuccessResponse,
    ProjectError,
};
use golem_api_grpc::proto::golem::project::Project;
use golem_common::metrics::api::TraceErrorKind;
use golem_common::model::auth::{AccountAction, ProjectAction};
use golem_common::model::{AccountId, ProjectId};
use golem_common::recorded_grpc_api_request;
use golem_common::SafeDisplay;
use golem_service_base::grpc::proto_project_id_string;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tonic::metadata::MetadataMap;
use tonic::{Request, Response, Status};
use tracing::Instrument;

impl From<AuthServiceError> for ProjectError {
    fn from(value: AuthServiceError) -> Self {
        let error = match value {
            AuthServiceError::InvalidToken(_)
            | AuthServiceError::AccountOwnershipRequired
            | AuthServiceError::RoleMissing { .. }
            | AuthServiceError::AccountAccessForbidden { .. }
            | AuthServiceError::ProjectAccessForbidden { .. }
            | AuthServiceError::ProjectActionForbidden { .. } => {
                project_error::Error::Unauthorized(ErrorBody {
                    error: value.to_safe_string(),
                })
            }
            AuthServiceError::InternalTokenServiceError(_)
            | AuthServiceError::InternalRepoError(_) => {
                project_error::Error::InternalError(ErrorBody {
                    error: value.to_safe_string(),
                })
            }
        };
        ProjectError { error: Some(error) }
    }
}

impl From<project::ProjectError> for ProjectError {
    fn from(value: project::ProjectError) -> Self {
        match value {
            project::ProjectError::InternalRepoError(_)
            | project::ProjectError::FailedToCreateDefaultProject(_)
            | project::ProjectError::InternalConversionError { .. }
            | project::ProjectError::InternalPlanLimitError(_) => {
                wrap_error(project_error::Error::InternalError(ErrorBody {
                    error: value.to_safe_string(),
                }))
            }
            project::ProjectError::LimitExceeded(_) => {
                wrap_error(project_error::Error::LimitExceeded(ErrorBody {
                    error: value.to_safe_string(),
                }))
            }
            project::ProjectError::ProjectNotFound { .. } => {
                wrap_error(project_error::Error::NotFound(ErrorBody {
                    error: value.to_safe_string(),
                }))
            }
            project::ProjectError::PluginNotFound { .. } => {
                wrap_error(project_error::Error::BadRequest(ErrorsBody {
                    errors: vec![value.to_safe_string()],
                }))
            }
            project::ProjectError::InternalPluginError(_) => {
                wrap_error(project_error::Error::InternalError(ErrorBody {
                    error: value.to_safe_string(),
                }))
            }
            project::ProjectError::CannotDeleteDefaultProject => {
                wrap_error(project_error::Error::BadRequest(ErrorsBody {
                    errors: vec![value.to_safe_string()],
                }))
            }
            project::ProjectError::InternalProjectAuthorisationError(inner) => inner.into(),
        }
    }
}

fn wrap_error(error: project_error::Error) -> ProjectError {
    ProjectError { error: Some(error) }
}

fn bad_request_error(error: &str) -> ProjectError {
    ProjectError {
        error: Some(project_error::Error::BadRequest(ErrorsBody {
            errors: vec![error.to_string()],
        })),
    }
}

pub struct ProjectGrpcApi {
    pub auth_service: Arc<dyn AuthService + Sync + Send>,
    pub project_service: Arc<dyn project::ProjectService + Sync + Send>,
}

impl ProjectGrpcApi {
    async fn auth(&self, metadata: MetadataMap) -> Result<AccountAuthorisation, ProjectError> {
        match get_authorisation_token(metadata) {
            Some(t) => self
                .auth_service
                .authorization(&t)
                .await
                .map_err(|e| e.into()),
            None => Err(ProjectError {
                error: Some(project_error::Error::Unauthorized(ErrorBody {
                    error: "Missing token".into(),
                })),
            }),
        }
    }

    async fn get_default(
        &self,
        _request: GetDefaultProjectRequest,
        metadata: MetadataMap,
    ) -> Result<Project, ProjectError> {
        let auth = self.auth(metadata).await?;
        let account_id = &auth.token.account_id;
        self.auth_service
            .authorize_account_action(&auth, account_id, &AccountAction::ViewDefaultProject)
            .await?;

        let result = self
            .project_service
            .get_default(&auth.token.account_id)
            .await?;
        Ok(result.into())
    }

    async fn get(
        &self,
        request: GetProjectRequest,
        metadata: MetadataMap,
    ) -> Result<Project, ProjectError> {
        let auth = self.auth(metadata).await?;

        let project_id: ProjectId = request
            .project_id
            .and_then(|id| id.try_into().ok())
            .ok_or_else(|| bad_request_error("Missing project id"))?;

        self.auth_service
            .authorize_project_action(&auth, &project_id, &ProjectAction::ViewProject)
            .await?;

        let result = self.project_service.get(&project_id).await?;

        match result {
            Some(project) => Ok(project.into()),
            None => Err(ProjectError {
                error: Some(project_error::Error::NotFound(ErrorBody {
                    error: "Project not found".to_string(),
                })),
            }),
        }
    }

    async fn get_all(
        &self,
        request: GetProjectsRequest,
        metadata: MetadataMap,
    ) -> Result<Vec<Project>, ProjectError> {
        let auth = self.auth(metadata).await?;
        let viewable_projects = self.auth_service.viewable_projects(&auth).await?;

        let projects = match request.project_name {
            Some(name) => {
                self.project_service
                    .get_all_by_name(&name, viewable_projects)
                    .await?
            }
            None => self.project_service.get_all(viewable_projects).await?,
        };

        Ok(projects.into_iter().map(|p| p.into()).collect())
    }

    async fn delete(
        &self,
        request: DeleteProjectRequest,
        metadata: MetadataMap,
    ) -> Result<(), ProjectError> {
        let auth = self.auth(metadata).await?;
        let project_id: ProjectId = request
            .project_id
            .and_then(|id| id.try_into().ok())
            .ok_or_else(|| bad_request_error("Missing project id"))?;

        self.auth_service
            .authorize_project_action(&auth, &project_id, &ProjectAction::DeleteProject)
            .await?;

        self.project_service.delete(&project_id).await?;

        Ok(())
    }

    async fn create(
        &self,
        request: CreateProjectRequest,
        metadata: MetadataMap,
    ) -> Result<Project, ProjectError> {
        let auth = self.auth(metadata).await?;

        let owner_account_id: AccountId = request
            .owner_account_id
            .map(|id| id.into())
            .ok_or_else(|| bad_request_error("Missing account id"))?;

        self.auth_service
            .authorize_account_action(&auth, &owner_account_id, &AccountAction::CreateProject)
            .await?;

        let project = model::Project {
            project_id: ProjectId::new_v4(),
            project_data: model::ProjectData {
                name: request.name,
                owner_account_id,
                description: request.description,
                default_environment_id: "default".to_string(),
                project_type: model::ProjectType::NonDefault,
            },
        };

        self.project_service.create(&project).await?;

        Ok(project.into())
    }
}

#[async_trait::async_trait]
impl CloudProjectService for ProjectGrpcApi {
    async fn get_default_project(
        &self,
        request: Request<GetDefaultProjectRequest>,
    ) -> Result<Response<GetDefaultProjectResponse>, Status> {
        let (m, _, r) = request.into_parts();

        let record = recorded_grpc_api_request!("get_default_project",);

        let response = match self.get_default(r, m).instrument(record.span.clone()).await {
            Ok(result) => record.succeed(get_default_project_response::Result::Success(result)),
            Err(error) => record.fail(
                get_default_project_response::Result::Error(error.clone()),
                &ProjectTraceErrorKind(&error),
            ),
        };

        Ok(Response::new(GetDefaultProjectResponse {
            result: Some(response),
        }))
    }

    async fn get_projects(
        &self,
        request: Request<GetProjectsRequest>,
    ) -> Result<Response<GetProjectsResponse>, Status> {
        let (m, _, r) = request.into_parts();

        let record = recorded_grpc_api_request!("get_projects", project_name = &r.project_name);

        let response = match self.get_all(r, m).instrument(record.span.clone()).await {
            Ok(data) => record.succeed(get_projects_response::Result::Success(
                GetProjectsSuccessResponse { data },
            )),
            Err(error) => record.fail(
                get_projects_response::Result::Error(error.clone()),
                &ProjectTraceErrorKind(&error),
            ),
        };

        Ok(Response::new(GetProjectsResponse {
            result: Some(response),
        }))
    }

    async fn create_project(
        &self,
        request: Request<CreateProjectRequest>,
    ) -> Result<Response<CreateProjectResponse>, Status> {
        let (m, _, r) = request.into_parts();

        let record = recorded_grpc_api_request!("create_project", project_name = r.name);

        let response = match self.create(r, m).instrument(record.span.clone()).await {
            Ok(result) => record.succeed(create_project_response::Result::Success(
                CreateProjectSuccessResponse {
                    project: Some(result),
                },
            )),
            Err(error) => record.fail(
                create_project_response::Result::Error(error.clone()),
                &ProjectTraceErrorKind(&error),
            ),
        };

        Ok(Response::new(CreateProjectResponse {
            result: Some(response),
        }))
    }

    async fn delete_project(
        &self,
        request: Request<DeleteProjectRequest>,
    ) -> Result<Response<DeleteProjectResponse>, Status> {
        let (m, _, r) = request.into_parts();

        let record = recorded_grpc_api_request!(
            "delete_project",
            project_id = proto_project_id_string(&r.project_id)
        );

        let response = match self.delete(r, m).instrument(record.span.clone()).await {
            Ok(_) => record.succeed(delete_project_response::Result::Success(Empty {})),
            Err(error) => record.fail(
                delete_project_response::Result::Error(error.clone()),
                &ProjectTraceErrorKind(&error),
            ),
        };

        Ok(Response::new(DeleteProjectResponse {
            result: Some(response),
        }))
    }

    async fn get_project(
        &self,
        request: Request<GetProjectRequest>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        let (m, _, r) = request.into_parts();

        let record = recorded_grpc_api_request!(
            "get_project",
            project_id = proto_project_id_string(&r.project_id)
        );

        let response = match self.get(r, m).instrument(record.span.clone()).await {
            Ok(result) => record.succeed(get_project_response::Result::Success(result)),
            Err(error) => record.fail(
                get_project_response::Result::Error(error.clone()),
                &ProjectTraceErrorKind(&error),
            ),
        };

        Ok(Response::new(GetProjectResponse {
            result: Some(response),
        }))
    }
}

pub struct ProjectTraceErrorKind<'a>(pub &'a ProjectError);

impl Debug for ProjectTraceErrorKind<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TraceErrorKind for ProjectTraceErrorKind<'_> {
    fn trace_error_kind(&self) -> &'static str {
        match &self.0.error {
            None => "None",
            Some(error) => match error {
                project_error::Error::BadRequest(_) => "BadRequest",
                project_error::Error::Unauthorized(_) => "Unauthorized",
                project_error::Error::LimitExceeded(_) => "LimitExceeded",
                project_error::Error::NotFound(_) => "NotFound",
                project_error::Error::InternalError(_) => "InternalError",
            },
        }
    }

    fn is_expected(&self) -> bool {
        match &self.0.error {
            None => false,
            Some(error) => match error {
                project_error::Error::BadRequest(_) => true,
                project_error::Error::Unauthorized(_) => true,
                project_error::Error::LimitExceeded(_) => true,
                project_error::Error::NotFound(_) => true,
                project_error::Error::InternalError(_) => false,
            },
        }
    }
}
