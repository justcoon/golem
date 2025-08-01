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

use super::ApiError;
use crate::api::ApiResult;
use crate::login::LoginSystem;
use crate::model::*;
use crate::service::auth::AuthService;
use crate::service::token::{TokenService, TokenServiceError};
use golem_common::model::auth::AccountAction;
use golem_common::model::error::ErrorBody;
use golem_common::model::AccountId;
use golem_common::model::TokenId;
use golem_common::recorded_http_api_request;
use golem_service_base::api_tags::ApiTags;
use golem_service_base::model::auth::GolemSecurityScheme;
use poem_openapi::param::Path;
use poem_openapi::payload::Json;
use poem_openapi::*;
use std::sync::Arc;
use tracing::Instrument;

pub struct TokenApi {
    pub auth_service: Arc<dyn AuthService>,
    pub token_service: Arc<dyn TokenService>,
    pub login_system: Arc<LoginSystem>,
}

#[OpenApi(prefix_path = "/v1/accounts", tag = ApiTags::Token)]
impl TokenApi {
    /// Get all tokens
    ///
    /// Gets all created tokens of an account.
    /// The format of each element is the same as the data object in the oauth2 endpoint's response.
    #[oai(
        path = "/:account_id/tokens",
        method = "get",
        operation_id = "get_tokens"
    )]
    async fn get_tokens(
        &self,
        account_id: Path<AccountId>,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<Vec<Token>>> {
        let record =
            recorded_http_api_request!("get_tokens", account_id = account_id.0.to_string());
        let response = self
            .get_tokens_internal(account_id.0, token)
            .instrument(record.span.clone())
            .await;

        record.result(response)
    }

    async fn get_tokens_internal(
        &self,
        account_id: AccountId,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<Vec<Token>>> {
        let auth = self.auth_service.authorization(token.as_ref()).await?;
        self.auth_service
            .authorize_account_action(&auth, &account_id, &AccountAction::ViewTokens)
            .await?;

        let result = self.token_service.find(&account_id).await?;
        Ok(Json(result))
    }

    #[allow(unused_variables)]
    #[oai(
        path = "/:account_id/tokens/:token_id",
        method = "get",
        operation_id = "get_token"
    )]
    /// Get a specific token
    ///
    /// Gets information about a token given by its identifier.
    /// The JSON is the same as the data object in the oauth2 endpoint's response.
    async fn get_token(
        &self,
        account_id: Path<AccountId>,
        token_id: Path<TokenId>,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<Token>> {
        let record = recorded_http_api_request!(
            "get_token",
            account_id = account_id.0.to_string(),
            token_id = token_id.0.to_string()
        );
        let response = self
            .get_token_internal(token_id.0, token)
            .instrument(record.span.clone())
            .await;

        record.result(response)
    }

    async fn get_token_internal(
        &self,
        token_id: TokenId,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<Token>> {
        let auth = self.auth_service.authorization(token.as_ref()).await?;
        let result = self.token_service.get(&token_id).await?;
        self.auth_service
            .authorize_account_action(&auth, &result.account_id, &AccountAction::ViewTokens)
            .await?;
        Ok(Json(result))
    }

    #[oai(
        path = "/:account_id/tokens",
        method = "post",
        operation_id = "create_token"
    )]
    /// Create new token
    ///
    /// Creates a new token with a given expiration date.
    /// The response not only contains the token data but also the secret which can be passed as a bearer token to the Authorization header to the Golem Cloud REST API.
    ///
    // Note that this is the only time this secret is returned!
    async fn post_token(
        &self,
        account_id: Path<AccountId>,
        request: Json<CreateTokenDTO>,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<UnsafeToken>> {
        let record =
            recorded_http_api_request!("create_token", account_id = account_id.0.to_string());
        let response = self
            .post_token_internal(account_id.0, request.0, token)
            .instrument(record.span.clone())
            .await;

        record.result(response)
    }

    async fn post_token_internal(
        &self,
        account_id: AccountId,
        request: CreateTokenDTO,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<UnsafeToken>> {
        let auth = self.auth_service.authorization(token.as_ref()).await?;
        self.auth_service
            .authorize_account_action(&auth, &account_id, &AccountAction::CreateToken)
            .await?;

        let response = self
            .token_service
            .create(&account_id, &request.expires_at)
            .await?;
        Ok(Json(response))
    }

    #[allow(unused_variables)]
    #[oai(
        path = "/:account_id/tokens/:token_id",
        method = "delete",
        operation_id = "delete_token"
    )]
    /// Delete a token
    ///
    /// Deletes a previously created token given by its identifier.
    async fn delete_token(
        &self,
        account_id: Path<AccountId>,
        token_id: Path<TokenId>,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<DeleteTokenResponse>> {
        let record = recorded_http_api_request!(
            "delete_token",
            account_id = account_id.0.to_string(),
            token_id = token_id.0.to_string()
        );
        let response = self
            .delete_token_internal(token_id.0, token)
            .instrument(record.span.clone())
            .await;

        record.result(response)
    }

    async fn delete_token_internal(
        &self,
        token_id: TokenId,
        token: GolemSecurityScheme,
    ) -> ApiResult<Json<DeleteTokenResponse>> {
        let auth = self.auth_service.authorization(token.as_ref()).await?;

        match self.token_service.get(&token_id).await {
            Ok(existing) => {
                self.auth_service
                    .authorize_account_action(
                        &auth,
                        &existing.account_id,
                        &AccountAction::DeleteToken,
                    )
                    .await?;

                if let LoginSystem::Enabled(login_system) = &*self.login_system {
                    login_system
                        .login_service
                        .unlink_temp_token(&token_id)
                        .await?;
                };

                self.token_service.delete(&token_id).await?;
                Ok(Json(DeleteTokenResponse {}))
            }
            Err(TokenServiceError::UnknownToken(_)) => {
                Err(ApiError::Unauthorized(Json(ErrorBody {
                    error: "Invalid token".to_string(),
                })))?
            }
            Err(e) => Err(e)?,
        }
    }
}
