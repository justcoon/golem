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

use crate::{FunctionTypeRegistry, InferredExpr, RegistryKey, RegistryValue, RibCompilationError};
use golem_wasm_ast::analysis::AnalysedType;

// An easier data type that focus just on the side effecting function calls in Rib script.
// These will not include variant or enum calls, that were originally
// tagged as functions before compilation.
// This is why we need a fully inferred Rib (fully compiled rib),
// which has specific details, along with original type registry to construct this data.
// These function calls are indeed worker invoke calls and nothing else.
// If Rib has inbuilt function support, those will not be included here either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerFunctionsInRib {
    pub function_calls: Vec<WorkerFunctionType>,
}

impl WorkerFunctionsInRib {
    pub fn from_inferred_expr(
        inferred_expr: &InferredExpr,
        original_type_registry: &FunctionTypeRegistry,
    ) -> Result<Option<WorkerFunctionsInRib>, RibCompilationError> {
        let worker_invoke_registry_keys = inferred_expr.worker_invoke_registry_keys();
        let type_registry_subset =
            original_type_registry.get_from_keys(worker_invoke_registry_keys);
        let mut function_calls = vec![];

        for (key, value) in type_registry_subset.types {
            if let RegistryValue::Function {
                parameter_types,
                return_type,
            } = value
            {
                let function_call_in_rib = WorkerFunctionType {
                    function_key: key,
                    parameter_types,
                    return_type,
                };
                function_calls.push(function_call_in_rib)
            } else {
                return Err(RibCompilationError::RibStaticAnalysisError(
                    "unable to inspect the worker function calls in rib. functional calls should have parameter types and return types".to_string(),
                ));
            }
        }

        if function_calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkerFunctionsInRib { function_calls }))
        }
    }
}

// The type of a function call with worker (ephmeral or durable) in Rib script
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerFunctionType {
    pub function_key: RegistryKey,
    pub parameter_types: Vec<AnalysedType>,
    pub return_type: Option<AnalysedType>,
}

#[cfg(feature = "protobuf")]
mod protobuf {
    use crate::{RegistryKey, WorkerFunctionType, WorkerFunctionsInRib};
    use golem_api_grpc::proto::golem::rib::WorkerFunctionType as WorkerFunctionTypeProto;
    use golem_api_grpc::proto::golem::rib::WorkerFunctionsInRib as WorkerFunctionsInRibProto;
    use golem_wasm_ast::analysis::AnalysedType;

    impl TryFrom<WorkerFunctionsInRibProto> for WorkerFunctionsInRib {
        type Error = String;

        fn try_from(value: WorkerFunctionsInRibProto) -> Result<Self, Self::Error> {
            let function_calls_proto = value.function_calls;
            let function_calls = function_calls_proto
                .iter()
                .map(|worker_function_type_proto| {
                    WorkerFunctionType::try_from(worker_function_type_proto.clone())
                })
                .collect::<Result<_, _>>()?;
            Ok(Self { function_calls })
        }
    }

    impl From<WorkerFunctionsInRib> for WorkerFunctionsInRibProto {
        fn from(value: WorkerFunctionsInRib) -> Self {
            WorkerFunctionsInRibProto {
                function_calls: value
                    .function_calls
                    .iter()
                    .map(|x| WorkerFunctionTypeProto::from(x.clone()))
                    .collect(),
            }
        }
    }

    impl TryFrom<WorkerFunctionTypeProto> for WorkerFunctionType {
        type Error = String;

        fn try_from(value: WorkerFunctionTypeProto) -> Result<Self, Self::Error> {
            let return_type = value
                .return_type
                .as_ref()
                .map(AnalysedType::try_from)
                .transpose()?;

            let parameter_types = value
                .parameter_types
                .iter()
                .map(AnalysedType::try_from)
                .collect::<Result<_, _>>()?;

            let registry_key_proto = value.function_key.ok_or("Function key missing")?;
            let function_key = RegistryKey::try_from(registry_key_proto)?;

            Ok(Self {
                function_key,
                return_type,
                parameter_types,
            })
        }
    }

    impl From<WorkerFunctionType> for WorkerFunctionTypeProto {
        fn from(value: WorkerFunctionType) -> Self {
            let registry_key = (&value.function_key).into();

            WorkerFunctionTypeProto {
                function_key: Some(registry_key),
                parameter_types: value
                    .parameter_types
                    .iter()
                    .map(|analysed_type| analysed_type.into())
                    .collect(),
                return_type: value
                    .return_type
                    .as_ref()
                    .map(|analysed_type| analysed_type.into()),
            }
        }
    }
}
