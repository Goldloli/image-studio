//! All objects here are contract fixtures. They do not describe actual inference.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use studio_core::{ContractError, contracts::*, domain::*};

fn asset(id: &str) -> AssetRef {
    AssetRef {
        asset_id: AssetId::new(id).unwrap(),
        content_hash: ContentHash::new("a".repeat(64)).unwrap(),
        data_type: AssetKind::ImageRgb,
        location: StorageLocation::Managed {
            storage_id: StorageLocationId::new("fixture-storage").unwrap(),
            relative_path: format!("fixtures/{id}.png"),
        },
    }
}

fn request() -> UserRequest {
    UserRequest {
        schema_version: REQUEST_SCHEMA_VERSION,
        request_id: RequestId::new("fixture-request").unwrap(),
        project_id: ProjectId::new("fixture-project").unwrap(),
        capability: CapabilityDescriptor {
            capability_id: "fixture.image.edit.v1".into(),
            instruction_required: true,
            input_roles: ["source", "identity", "style"]
                .into_iter()
                .map(|role| RoleRule {
                    role: role.into(),
                    minimum: u32::from(role == "source"),
                    maximum: 2,
                })
                .collect(),
        },
        instruction: "仅用于契约测试：保留人物，调整背景。\nContract fixture only.".into(),
        inputs: ["source", "identity", "style"]
            .into_iter()
            .enumerate()
            .map(|(index, role)| InputBinding {
                asset: asset(&format!("fixture-input-{index}")),
                role: role.into(),
                order: index as u32,
            })
            .collect(),
        model_variant_id: ModelVariantId::new("fixture-model").unwrap(),
        requested_output: ImageOutput {
            width: 1024,
            height: 1024,
            count: 2,
        },
        policy: AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
        },
        parameters: BTreeMap::from([("seed".into(), json!(1234))]),
        idempotency_key: IdempotencyKey::new("fixture-action").unwrap(),
    }
}

fn plan(request: &UserRequest) -> ExecutionPlan {
    ExecutionPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan_id: PlanId::new("fixture-plan").unwrap(),
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: vec![ComponentSnapshot {
            role: "main_network".into(),
            source: SourceRevision {
                source: "https://fixture.invalid/model".into(),
                revision: "fixture-model-revision".into(),
            },
            content_hash: ContentHash::new("b".repeat(64)).unwrap(),
        }],
        runtime: RuntimeSnapshot {
            package_id: "fixture-runtime".into(),
            engine: "contract-fixture".into(),
            source: SourceRevision {
                source: "https://fixture.invalid/runtime".into(),
                revision: "fixture-runtime-revision".into(),
            },
        },
        device: DeviceSnapshot {
            device_id: "fixture-device".into(),
            precision: "fixture-precision".into(),
        },
        effective_output: request.requested_output.clone(),
        effective_parameters: request.parameters.clone(),
        adjustments: vec![],
    }
}

fn attempt(id: &str, token: &str) -> Attempt {
    Attempt {
        attempt_id: AttemptId::new(id).unwrap(),
        job_id: JobId::new("fixture-job").unwrap(),
        execution_token: ExecutionToken::new(token).unwrap(),
    }
}

fn manifest() -> RunManifest {
    let request = request();
    RunManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        evidence_kind: EvidenceKind::ContractFixture,
        job_id: JobId::new("fixture-job").unwrap(),
        attempt: attempt("fixture-attempt", "fixture-token"),
        plan: plan(&request),
        input_assets: request
            .inputs
            .iter()
            .map(|input| input.asset.clone())
            .collect(),
        request,
        outputs: vec![asset("fixture-output")],
        final_result: FinalResult::Succeeded,
    }
}

fn value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}
fn json_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}

#[test]
fn output_cannot_reuse_an_input_identity_with_different_bytes() {
    let mut document = manifest();
    document.outputs = vec![document.input_assets[0].clone()];
    document.validate().unwrap();
    document.outputs[0].content_hash = ContentHash::new("b".repeat(64)).unwrap();
    assert!(document.validate().is_err());
}

#[test]
fn ids_reject_paths_empty_strings_and_display_text() {
    for invalid in [
        "",
        "/Users/a/picture.png",
        r"C:\Pictures\image.png",
        "display name",
        "../asset",
    ] {
        assert!(AssetId::new(invalid).is_err(), "{invalid}");
    }
    assert_eq!(ProjectId::new("project-1").unwrap().as_str(), "project-1");
    assert_eq!(
        ContentHash::new("A".repeat(64)).unwrap().as_str(),
        "a".repeat(64)
    );
    assert!(ContentHash::new("g".repeat(64)).is_err());
}

#[test]
fn changing_location_does_not_change_asset_identity_or_content() {
    let mut moved = asset("fixture-asset");
    let original = moved.clone();
    moved.location = StorageLocation::External {
        path: r"D:\图库\image.png".into(),
    };
    moved.validate().unwrap();
    assert_eq!(moved.asset_id, original.asset_id);
    assert_eq!(moved.content_hash, original.content_hash);
    assert_ne!(moved.location, original.location);
}

#[test]
fn managed_paths_reject_traversal_but_external_locations_are_descriptions() {
    for path in [
        "../outside.png",
        "/absolute.png",
        "a//b.png",
        r"a\b.png",
        "C:relative.png",
        "a/./b.png",
    ] {
        let location = StorageLocation::Managed {
            storage_id: StorageLocationId::new("fixture-storage").unwrap(),
            relative_path: path.into(),
        };
        assert!(location.validate().is_err(), "{path}");
    }
}

#[test]
fn current_document_versions_round_trip_every_defined_field() {
    let request = request();
    let plan = plan(&request);
    let manifest = manifest();
    assert_eq!(
        UserRequest::from_json(&request.to_json().unwrap()).unwrap(),
        request
    );
    assert_eq!(
        ExecutionPlan::from_json(&plan.to_json().unwrap()).unwrap(),
        plan
    );
    let json = manifest.to_json().unwrap();
    assert!(json.contains("contract_fixture"));
    assert_eq!(RunManifest::from_json(&json).unwrap(), manifest);
}

#[test]
fn future_versions_are_rejected_before_current_fields_are_interpreted() {
    let mut request = value(&request());
    request["schema_version"] = json!(2);
    request["unknown_future_field"] = json!(true);
    assert!(matches!(
        UserRequest::from_json(&json_string(&request)),
        Err(ContractError::UnsupportedVersion {
            document: "user_request",
            found: 2,
            ..
        })
    ));
    let mut plan = value(&plan(&self::request()));
    plan["schema_version"] = json!(2);
    assert!(matches!(
        ExecutionPlan::from_json(&json_string(&plan)),
        Err(ContractError::UnsupportedVersion {
            document: "execution_plan",
            ..
        })
    ));
    let mut manifest = value(&manifest());
    manifest["schema_version"] = json!(2);
    assert!(matches!(
        RunManifest::from_json(&json_string(&manifest)),
        Err(ContractError::UnsupportedVersion {
            document: "run_manifest",
            ..
        })
    ));
}

#[test]
fn missing_unknown_invalid_and_malformed_values_have_distinct_errors() {
    let original = value(&request());
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("instruction");
    assert_eq!(
        UserRequest::from_json(&json_string(&missing)).unwrap_err(),
        ContractError::MissingField("instruction".into())
    );
    let mut unknown = original.clone();
    unknown["silent_override"] = json!(true);
    assert_eq!(
        UserRequest::from_json(&json_string(&unknown)).unwrap_err(),
        ContractError::UnknownField("silent_override".into())
    );
    let mut invalid = original;
    invalid["requested_output"]["width"] = json!(0);
    assert!(matches!(
        UserRequest::from_json(&json_string(&invalid)),
        Err(ContractError::InvalidValue(_))
    ));
    assert!(matches!(
        UserRequest::from_json("{"),
        Err(ContractError::InvalidJson(_))
    ));
}

#[test]
fn nested_unknown_fields_and_missing_hashes_are_rejected() {
    let mut request = value(&request());
    request["policy"]["run_python"] = json!("bad");
    assert!(matches!(
        UserRequest::from_json(&json_string(&request)),
        Err(ContractError::UnknownField(_))
    ));
    let mut manifest = value(&manifest());
    manifest["outputs"][0]
        .as_object_mut()
        .unwrap()
        .remove("content_hash");
    assert_eq!(
        RunManifest::from_json(&json_string(&manifest)).unwrap_err(),
        ContractError::MissingField("content_hash".into())
    );
}

#[test]
fn input_roles_and_orders_survive_without_reordering() {
    let mut request = request();
    request.inputs.swap(0, 2);
    let decoded = UserRequest::from_json(&request.to_json().unwrap()).unwrap();
    assert_eq!(decoded.inputs, request.inputs);
    assert_eq!(decoded.inputs[0].role, "style");
    assert_eq!(decoded.inputs[0].order, 2);
}

#[test]
fn capability_rejects_unknown_roles_duplicates_and_invalid_counts() {
    let mut invalid = request();
    invalid.inputs[0].role = "unregistered".into();
    assert!(invalid.validate().is_err());
    let mut invalid = request();
    invalid.inputs[1].order = invalid.inputs[0].order;
    assert!(invalid.validate().is_err());
    let mut invalid = request();
    invalid.inputs.remove(0);
    assert!(invalid.validate().is_err());
    let mut invalid = request();
    invalid.capability.input_roles[0].maximum = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = request();
    invalid
        .capability
        .input_roles
        .push(invalid.capability.input_roles[0].clone());
    assert!(invalid.validate().is_err());
}

#[test]
fn one_input_identity_cannot_claim_two_hashes() {
    let mut request = request();
    request.inputs[1].asset.asset_id = request.inputs[0].asset.asset_id.clone();
    request.inputs[1].asset.content_hash = ContentHash::new("f".repeat(64)).unwrap();
    assert!(request.validate().is_err());
}

#[test]
fn resolution_adjustment_preserves_original_and_requires_permission_and_evidence() {
    let mut request = request();
    let mut plan = plan(&request);
    plan.effective_output.width = 768;
    assert!(plan.validate_against(&request).is_err());
    request.policy.allow_resolution_change = true;
    assert!(plan.validate_against(&request).is_err());
    plan.adjustments.push(PlanAdjustment {
        field: "output".into(),
        requested: value(&request.requested_output),
        effective: value(&plan.effective_output),
        reason: "Contract fixture size adaptation".into(),
    });
    plan.validate_against(&request).unwrap();
    assert_eq!(request.requested_output.width, 1024);
    assert_eq!(plan.effective_output.width, 768);
    assert!(request.parameters.contains_key("seed"));
}

#[test]
fn model_substitution_and_count_changes_cannot_be_silent() {
    let mut request = request();
    let mut plan = plan(&request);
    plan.model_variant_id = ModelVariantId::new("fixture-other-model").unwrap();
    assert!(plan.validate_against(&request).is_err());
    request.policy.allow_model_substitution = true;
    plan.adjustments.push(PlanAdjustment {
        field: "model_variant_id".into(),
        requested: json!(request.model_variant_id.as_str()),
        effective: json!(plan.model_variant_id.as_str()),
        reason: "Contract fixture substitution".into(),
    });
    plan.validate_against(&request).unwrap();
    plan.effective_output.count += 1;
    assert!(plan.validate_against(&request).is_err());
}

#[test]
fn retry_preserves_job_and_old_attempt_while_fencing_old_events() {
    let old = attempt("fixture-old", "fixture-token-old");
    let current = attempt("fixture-new", "fixture-token-new");
    let mut job = Job::new(old.job_id.clone());
    assert_eq!(
        job.validate_event(&old),
        Err(ContractError::NoActiveAttempt)
    );
    job.add_attempt(old.clone()).unwrap();
    job.add_attempt(current.clone()).unwrap();
    assert_eq!(job.attempts(), &[old.clone(), current.clone()]);
    assert_eq!(job.validate_event(&old), Err(ContractError::StaleAttempt));
    job.validate_event(&current).unwrap();
    let mut wrong_token = current.clone();
    wrong_token.execution_token = old.execution_token.clone();
    assert_eq!(
        job.validate_event(&wrong_token),
        Err(ContractError::StaleToken)
    );
    assert!(job.add_attempt(current.clone()).is_err());
    let mut reused_token = attempt("fixture-another", "fixture-token-new");
    assert!(job.add_attempt(reused_token.clone()).is_err());
    reused_token.execution_token = ExecutionToken::new("fixture-unique-token").unwrap();
    reused_token.job_id = JobId::new("fixture-other-job").unwrap();
    assert!(job.add_attempt(reused_token).is_err());
}

#[test]
fn snapshots_do_not_requery_mutable_catalog_information() {
    let manifest = manifest();
    let saved = manifest.to_json().unwrap();
    let mut external_catalog_entry = manifest.plan.components[0].clone();
    external_catalog_entry.source.revision = "new-directory-revision".into();
    let decoded = RunManifest::from_json(&saved).unwrap();
    assert_eq!(decoded, manifest);
    assert_ne!(decoded.plan.components[0], external_catalog_entry);
}

#[test]
fn successful_manifests_require_outputs_and_complete_input_identity() {
    let mut manifest = manifest();
    manifest.outputs.clear();
    assert!(manifest.validate().is_err());
    manifest.final_result = FinalResult::Failed {
        code: "FIXTURE_FAILURE".into(),
    };
    manifest.validate().unwrap();
    manifest.input_assets[0].content_hash = ContentHash::new("c".repeat(64)).unwrap();
    assert!(manifest.validate().is_err());
}

#[test]
fn independent_nested_versions_and_plan_request_links_are_checked() {
    let mut manifest = manifest();
    manifest.request.schema_version = 2;
    assert!(matches!(
        manifest.validate(),
        Err(ContractError::UnsupportedVersion {
            document: "user_request",
            ..
        })
    ));
    manifest.request.schema_version = REQUEST_SCHEMA_VERSION;
    manifest.plan.request_id = RequestId::new("fixture-other-request").unwrap();
    assert!(manifest.validate().is_err());
}
