use agentbaton_workspaces::{
    deterministic::*,
    validation::{
        sha256, AnalysisReproducibility, AnalysisScope, ArtifactRequirement, BoundArtifact,
        ValidationError, ValidationSubject, VersionBinding,
    },
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, os::unix::fs::symlink, path::PathBuf};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap().join("artifacts");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("dataset"), b"fixed source snapshot\n").unwrap();
        // Executable-looking output is hashed, never run.
        fs::write(
            root.join("reproduce.sh"),
            b"#!/bin/sh\ntouch MUST_NOT_EXIST\n",
        )
        .unwrap();
        fs::write(root.join("environment.json"), b"{\"engine\":\"fixture\"}\n").unwrap();
        Self {
            _directory: directory,
            root,
        }
    }
    fn subject(&self) -> ValidationSubject {
        ValidationSubject {
            task_id: "task-1".into(),
            attempt_id: "attempt-1".into(),
            spec_revision: 1,
            input_revision: "input-v1".into(),
            version: VersionBinding::Data {
                dataset_revision: "dataset-v1".into(),
            },
        }
    }
    fn contract(&self) -> JsonTableContract {
        JsonTableContract {
            table_artifact_id: "table".into(),
            dataset_rows: 2,
            columns: vec![
                Column {
                    name: "x".into(),
                    required: true,
                    nullable: false,
                    scalar: ScalarType::Number {
                        min: -10.0,
                        max: 10.0,
                    },
                },
                Column {
                    name: "n".into(),
                    required: true,
                    nullable: false,
                    scalar: ScalarType::Integer {
                        min: i64::MIN,
                        max: i64::MAX,
                    },
                },
                Column {
                    name: "valid".into(),
                    required: true,
                    nullable: false,
                    scalar: ScalarType::Boolean,
                },
                Column {
                    name: "label".into(),
                    required: false,
                    nullable: true,
                    scalar: ScalarType::String { max_bytes: 16 },
                },
            ],
            analysis: AnalysisReproducibility {
                script_artifact_id: "script".into(),
                environment_artifact_id: "environment".into(),
                units: BTreeMap::from([("x".into(), "m".into()), ("n".into(), "count".into())]),
                timezone: "UTC".into(),
                seed: Some(7),
                scope: AnalysisScope::Full,
            },
            limits: JsonLimits::default(),
        }
    }
    fn artifact(&self, id: &str, path: &str, expected: Option<String>) -> BoundArtifact {
        BoundArtifact::from_approved_root(
            &self.root,
            ArtifactRequirement {
                artifact_id: id.into(),
                relative_path: path.into(),
                max_bytes: 1_048_576,
                expected_sha256: expected,
            },
        )
        .unwrap()
    }
    fn inputs(&self) -> Vec<BoundArtifact> {
        vec![self.artifact("input", "dataset", Some(sha256(b"fixed source snapshot\n")))]
    }
    fn outputs(&self) -> Vec<BoundArtifact> {
        vec![
            self.artifact("table", "table.json", None),
            self.artifact("script", "reproduce.sh", None),
            self.artifact("environment", "environment.json", None),
        ]
    }
    fn rule(&self, contract: JsonTableContract) -> ApprovedJsonTableRule {
        self.try_rule(contract).unwrap()
    }
    fn try_rule(
        &self,
        contract: JsonTableContract,
    ) -> Result<ApprovedJsonTableRule, ValidationError> {
        ApprovedJsonTableRule::from_verified_policy(
            "owner-1".into(),
            "policy-1".into(),
            self.subject(),
            self.inputs(),
            self.outputs(),
            contract,
        )
    }
    fn document(&self, contract: &JsonTableContract) -> Value {
        let rows = match contract.analysis.scope {
            AnalysisScope::Full => contract.dataset_rows,
            AnalysisScope::Sample { rows, .. } => rows,
        };
        json!({"metadata":{
            "input_revision":"input-v1","dataset_revision":"dataset-v1",
            "input_digests":{"input":sha256(b"fixed source snapshot\n")},
            "table_artifact_id":"table","row_count":rows,"dataset_rows":contract.dataset_rows,
            "units":contract.analysis.units,"timezone":contract.analysis.timezone,"seed":contract.analysis.seed,"scope":contract.analysis.scope,
            "script_sha256":sha256(&fs::read(self.root.join("reproduce.sh")).unwrap()),
            "environment_sha256":sha256(&fs::read(self.root.join("environment.json")).unwrap())
        },"rows":(0..rows).map(|n|json!({"x":1.5,"n":n,"valid":true})).collect::<Vec<_>>()})
    }
    fn write(&self, value: &Value) {
        fs::write(
            self.root.join("table.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }
    fn raw(&self, bytes: &[u8]) {
        fs::write(self.root.join("table.json"), bytes).unwrap();
    }
    fn outcome(&self, contract: JsonTableContract, value: &Value) -> JsonTableEvidence {
        self.write(value);
        validate_json_table(&self.rule(contract)).unwrap()
    }
}
fn code(evidence: &JsonTableEvidence) -> FailureCode {
    evidence.view().failure.unwrap().code
}

#[test]
fn actual_contract_pass_is_reproducible_and_not_task_or_script_acceptance() {
    let f = Fixture::new();
    let c = f.contract();
    f.write(&f.document(&c));
    let rule = f.rule(c);
    let first = validate_json_table(&rule).unwrap();
    let second = validate_json_table(&rule).unwrap();
    assert!(first.view().passed);
    assert!(first.view().file_integrity.passed);
    assert_eq!(first.view().checked_rows, Some(2));
    assert_eq!(first.view().digest, second.view().digest);
    let accepted = first.accept_contract(&rule).unwrap();
    assert_eq!(accepted.contract_digest(), rule.contract_digest());
    assert_eq!(accepted.subject(), &f.subject());
    assert_eq!(accepted.validation_digest(), first.view().digest);
    assert!(!f.root.join("MUST_NOT_EXIST").exists());
    assert_eq!(first.view().validator_id, VALIDATOR_ID);
    assert_eq!(
        first.view().file_integrity.validator_id,
        "builtin.artifact-integrity.v1"
    );
}
#[test]
fn strict_schema_checks_required_nullable_optional_unknown_and_scalar_types() {
    let f = Fixture::new();
    let c = f.contract();
    let original = f.document(&c);
    let mut nullable = original.clone();
    nullable["rows"][0]["label"] = Value::Null;
    assert!(f.outcome(c.clone(), &nullable).view().passed);
    for (field, value, expected) in [
        ("x", json!("1.5"), FailureCode::FieldType),
        ("x", Value::Null, FailureCode::FieldType),
        ("n", json!(1.0), FailureCode::FieldType),
        ("valid", json!(1), FailureCode::FieldType),
        ("label", json!(false), FailureCode::FieldType),
        ("label", json!("x".repeat(17)), FailureCode::StringLength),
    ] {
        let mut v = original.clone();
        v["rows"][0][field] = value;
        let e = f.outcome(c.clone(), &v);
        assert_eq!(code(&e), expected);
        assert!(!e.view().passed);
    }
    let mut missing = original.clone();
    missing["rows"][0].as_object_mut().unwrap().remove("x");
    assert_eq!(
        code(&f.outcome(c.clone(), &missing)),
        FailureCode::MissingField
    );
    let mut extra = original;
    extra["rows"][0]["unapproved_rule"] = json!(true);
    assert_eq!(code(&f.outcome(c, &extra)), FailureCode::UnexpectedField);
}
#[test]
fn inclusive_ranges_and_signed_integer_boundaries_do_not_use_float_coercion() {
    let f = Fixture::new();
    let c = f.contract();
    let original = f.document(&c);
    for number in [-10.0, 10.0] {
        let mut v = original.clone();
        v["rows"][0]["x"] = json!(number);
        assert!(f.outcome(c.clone(), &v).view().passed);
    }
    for number in [-10.01, 10.01] {
        let mut v = original.clone();
        v["rows"][0]["x"] = json!(number);
        assert_eq!(code(&f.outcome(c.clone(), &v)), FailureCode::NumericRange);
    }
    for number in [i64::MIN, i64::MAX] {
        let mut v = original.clone();
        v["rows"][0]["n"] = json!(number);
        assert!(f.outcome(c.clone(), &v).view().passed);
    }
    let mut v = original;
    v["rows"][0]["n"] = json!(u64::MAX);
    assert_eq!(code(&f.outcome(c, &v)), FailureCode::FieldType);
}
#[test]
fn syntax_nonfinite_duplicate_keys_and_trailing_data_are_rejected_without_echoing_values() {
    let f = Fixture::new();
    let rule = f.rule(f.contract());
    for bytes in [
        b"".as_slice(),
        b"{} trailing",
        b"{\"rows\":[NaN]}",
        b"{\"rows\":[Infinity]}",
        b"{\"rows\":[1e999]}",
        b"{\"x\":\"\xff\"}",
    ] {
        f.raw(bytes);
        let e = validate_json_table(&rule).unwrap();
        assert_eq!(code(&e), FailureCode::JsonSyntax);
        assert!(e.view().failure.unwrap().line.is_some());
    }
    for bytes in [
        b"{\"rows\":[],\"rows\":[]}".as_slice(),
        b"{\"rows\":[],\"r\\u006fws\":[]}",
        b"{\"rows\":[{\"x\":1,\"x\":2}]}",
    ] {
        f.raw(bytes);
        assert_eq!(
            code(&validate_json_table(&rule).unwrap()),
            FailureCode::JsonDuplicateKey
        );
    }
    let mut v = f.document(&f.contract());
    v["rows"][0]["x"] = json!("SECRET_UNTRUSTED_VALUE");
    f.write(&v);
    let e = validate_json_table(&rule).unwrap();
    assert!(!serde_json::to_string(&e.view())
        .unwrap()
        .contains("SECRET_UNTRUSTED_VALUE"));
}
#[test]
fn exact_units_input_bindings_reproduction_and_sampling_cannot_be_self_upgraded() {
    let f = Fixture::new();
    let mut c = f.contract();
    c.dataset_rows = 100;
    c.analysis.scope = AnalysisScope::Sample {
        rows: 2,
        limitation: "Only the pinned sample, not population conclusions".into(),
    };
    let original = f.document(&c);
    assert!(f.outcome(c.clone(), &original).view().passed);
    for (field, value) in [
        ("units", json!({"x":"km","n":"count"})),
        ("scope", json!({"kind":"full"})),
        ("seed", json!(8)),
        ("input_revision", json!("other")),
        ("dataset_revision", json!("other")),
        ("dataset_rows", json!(2)),
        ("row_count", json!(100)),
        ("timezone", json!("local")),
        ("script_sha256", json!("0".repeat(64))),
        ("environment_sha256", json!("0".repeat(64))),
        ("input_digests", json!({"input":"0".repeat(64)})),
    ] {
        let mut v = original.clone();
        v["metadata"][field] = value;
        let e = f.outcome(c.clone(), &v);
        assert_eq!(code(&e), FailureCode::MetadataMismatch, "{field}");
    }
    let mut extra = original;
    extra["metadata"]["passed"] = json!(true);
    assert_eq!(code(&f.outcome(c, &extra)), FailureCode::MetadataMismatch);
}
#[test]
fn row_count_shape_and_root_self_approval_are_not_trusted() {
    let f = Fixture::new();
    let c = f.contract();
    let original = f.document(&c);
    let mut short = original.clone();
    short["rows"].as_array_mut().unwrap().pop();
    assert_eq!(code(&f.outcome(c.clone(), &short)), FailureCode::RowCount);
    let mut bad = original.clone();
    bad["rows"][0] = json!([]);
    assert_eq!(code(&f.outcome(c.clone(), &bad)), FailureCode::RowsShape);
    let mut bad = original.clone();
    bad["rows"] = json!({});
    assert_eq!(code(&f.outcome(c.clone(), &bad)), FailureCode::RowsShape);
    let mut bad = original;
    bad["approved_rule"] = json!({"passed":true,"limits":"unlimited"});
    assert_eq!(code(&f.outcome(c.clone(), &bad)), FailureCode::RootShape);
    assert_eq!(code(&f.outcome(c, &json!([1, 2]))), FailureCode::RootShape);
}
fn values(v: &Value) -> u32 {
    1 + match v {
        Value::Array(a) => a.iter().map(values).sum(),
        Value::Object(m) => m.values().map(|v| 1 + values(v)).sum(),
        _ => 0,
    }
}
#[test]
fn parser_depth_values_and_decoded_utf8_string_budgets_have_exact_boundaries() {
    let f = Fixture::new();
    let mut c = f.contract();
    let original = f.document(&c);
    let count = values(&original);
    c.limits.max_values = count;
    assert!(f.outcome(c.clone(), &original).view().passed);
    c.limits.max_values = count - 1;
    assert_eq!(
        code(&f.outcome(c.clone(), &original)),
        FailureCode::JsonValueLimit
    );
    c.limits.max_values = count;
    c.limits.max_depth = 4;
    assert!(f.outcome(c.clone(), &original).view().passed);
    c.limits.max_depth = 3;
    assert_eq!(
        code(&f.outcome(c.clone(), &original)),
        FailureCode::JsonDepthLimit
    );
    c.limits.max_depth = 16;
    c.limits.max_values = JsonLimits::default().max_values;
    c.limits.max_string_bytes = 64;
    c.columns
        .iter_mut()
        .find(|v| v.name == "label")
        .unwrap()
        .scalar = ScalarType::String { max_bytes: 64 };
    let mut text = original.clone();
    text["rows"][0]["label"] = json!("é".repeat(32));
    assert!(f.outcome(c.clone(), &text).view().passed);
    text["rows"][0]["label"] = json!("é".repeat(33));
    assert_eq!(code(&f.outcome(c, &text)), FailureCode::JsonStringLimit);
}
#[test]
fn full_zero_rows_and_explicit_null_seed_are_supported_but_sample_zero_is_not() {
    let f = Fixture::new();
    let mut c = f.contract();
    c.dataset_rows = 0;
    c.analysis.seed = None;
    assert!(f.outcome(c.clone(), &f.document(&c)).view().passed);
    c.dataset_rows = 2;
    c.analysis.scope = AnalysisScope::Sample {
        rows: 0,
        limitation: "empty".into(),
    };
    assert!(f.try_rule(c).is_err());
}
#[test]
fn invalid_rule_limits_ranges_names_units_and_aliases_fail_before_reading() {
    let f = Fixture::new();
    let original = f.contract();
    for choice in 0..12 {
        let mut c = original.clone();
        match choice {
            0 => c.limits.max_bytes = 4_194_305,
            1 => c.limits.max_depth = 33,
            2 => c.limits.max_values = 0,
            3 => c.limits.max_rows = 0,
            4 => {
                c.columns[0].scalar = ScalarType::Number {
                    min: f64::NAN,
                    max: 1.0,
                }
            }
            5 => {
                c.columns[0].scalar = ScalarType::Number {
                    min: 1.0,
                    max: f64::INFINITY,
                }
            }
            6 => c.columns[0].scalar = ScalarType::Integer { min: 2, max: 1 },
            7 => c.columns[0].name = "../bad".into(),
            8 => c.columns.push(c.columns[0].clone()),
            9 => {
                c.analysis.units.insert("valid".into(), "unit".into());
            }
            10 => c.table_artifact_id = c.analysis.script_artifact_id.clone(),
            11 => c.dataset_rows = 100_000,
            _ => unreachable!(),
        }
        assert!(
            matches!(f.try_rule(c), Err(ValidationError::InvalidContract)),
            "case {choice}"
        );
    }
    let mut outputs = f.outputs();
    outputs[1] = f.artifact("script", "table.json", None);
    assert!(ApprovedJsonTableRule::from_verified_policy(
        "owner".into(),
        "policy".into(),
        f.subject(),
        f.inputs(),
        outputs,
        original
    )
    .is_err());
}
#[test]
fn missing_or_oversized_artifacts_fail_even_if_output_reports_success() {
    let f = Fixture::new();
    let c = f.contract();
    f.write(&f.document(&c));
    let rule = f.rule(c);
    fs::remove_file(f.root.join("reproduce.sh")).unwrap();
    let e = validate_json_table(&rule).unwrap();
    assert_eq!(code(&e), FailureCode::ArtifactIntegrity);
    assert!(matches!(
        e.accept_contract(&rule),
        Err(ValidationError::ValidationFailed)
    ));
    fs::write(f.root.join("reproduce.sh"), vec![b'x'; 1_048_577]).unwrap();
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::ArtifactIntegrity
    );
}
#[test]
fn symlinks_hardlinks_and_root_replacement_cannot_escape_bound_artifacts() {
    let f = Fixture::new();
    let c = f.contract();
    f.write(&f.document(&c));
    let rule = f.rule(c.clone());
    let saved = f.root.join("saved-table");
    fs::rename(f.root.join("table.json"), &saved).unwrap();
    symlink(&saved, f.root.join("table.json")).unwrap();
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::ArtifactIntegrity
    );
    fs::remove_file(f.root.join("table.json")).unwrap();
    fs::hard_link(&saved, f.root.join("table.json")).unwrap();
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::ArtifactIntegrity
    );
    fs::remove_file(f.root.join("table.json")).unwrap();
    fs::rename(&saved, f.root.join("table.json")).unwrap();
    let e = validate_json_table(&rule).unwrap();
    assert!(e.view().passed);
    let moved = f.root.with_file_name("moved");
    fs::rename(&f.root, &moved).unwrap();
    fs::create_dir(&f.root).unwrap();
    assert!(matches!(
        e.verify_current(),
        Err(ValidationError::StaleEvidence)
    ));
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::ArtifactIntegrity
    );
}
#[test]
fn traversal_is_rejected_by_reused_bound_artifact_constructor() {
    let f = Fixture::new();
    assert!(BoundArtifact::from_approved_root(
        &f.root,
        ArtifactRequirement {
            artifact_id: "bad".into(),
            relative_path: "../outside".into(),
            max_bytes: 10,
            expected_sha256: None
        }
    )
    .is_err());
}
#[test]
fn stale_input_table_script_environment_bytes_invalidate_prior_contract_acceptance() {
    for path in ["dataset", "table.json", "reproduce.sh", "environment.json"] {
        let f = Fixture::new();
        let c = f.contract();
        f.write(&f.document(&c));
        let rule = f.rule(c);
        let e = validate_json_table(&rule).unwrap();
        assert!(e.view().passed);
        fs::write(f.root.join(path), b"changed").unwrap();
        assert!(
            matches!(
                e.accept_contract(&rule),
                Err(ValidationError::StaleEvidence)
            ),
            "{path}"
        );
    }
}
#[test]
fn output_bindings_subject_schema_policy_and_approval_drift_require_new_evidence() {
    let f = Fixture::new();
    let original = f.contract();
    f.write(&f.document(&original));
    let rule = f.rule(original.clone());
    let evidence = validate_json_table(&rule).unwrap();
    for choice in 0..9 {
        let mut c = original.clone();
        let mut subject = f.subject();
        let mut id = "owner-1";
        let mut revision = "policy-1";
        let mut outputs = f.outputs();
        match choice {
            0 => subject.attempt_id = "attempt-2".into(),
            1 => subject.spec_revision = 2,
            2 => subject.input_revision = "input-v2".into(),
            3 => {
                subject.version = VersionBinding::Data {
                    dataset_revision: "dataset-v2".into(),
                }
            }
            4 => {
                c.columns[0].scalar = ScalarType::Number {
                    min: -9.0,
                    max: 9.0,
                }
            }
            5 => {
                c.analysis.units.insert("x".into(), "km".into());
            }
            6 => id = "owner-2",
            7 => revision = "policy-2",
            8 => outputs[0] = f.artifact("table", "different.json", None),
            _ => unreachable!(),
        }
        let different = ApprovedJsonTableRule::from_verified_policy(
            id.into(),
            revision.into(),
            subject,
            f.inputs(),
            outputs,
            c,
        )
        .unwrap();
        assert!(
            matches!(
                evidence.accept_contract(&different),
                Err(ValidationError::ApprovalMismatch)
            ),
            "{choice}"
        );
    }
}
#[test]
fn proposal_column_order_is_canonical_but_mutating_receipt_view_never_grants_acceptance() {
    let f = Fixture::new();
    let c = f.contract();
    let one = f.rule(c.clone());
    let mut reverse = c.clone();
    reverse.columns.reverse();
    let two = f.rule(reverse);
    assert_eq!(one.contract_digest(), two.contract_digest());
    let mut bad = f.document(&c);
    bad["rows"][0]["x"] = json!(999);
    let e = f.outcome(c, &bad);
    assert!(e.view().file_integrity.passed);
    assert!(!e.view().passed);
    let mut counterfeit = e.view();
    counterfeit.passed = true;
    counterfeit.failure = None;
    assert!(matches!(
        e.accept_contract(&one),
        Err(ValidationError::ValidationFailed)
    ));
    assert!(counterfeit.passed);
}

#[test]
fn all_required_input_hashes_must_be_present_and_unpinned_inputs_are_not_approved() {
    let f = Fixture::new();
    let c = f.contract();
    let mut document = f.document(&c);
    fs::write(f.root.join("dataset2"), b"second snapshot").unwrap();
    let mut inputs = f.inputs();
    inputs.push(f.artifact("input2", "dataset2", Some(sha256(b"second snapshot"))));
    let rule = ApprovedJsonTableRule::from_verified_policy(
        "owner-1".into(),
        "policy-1".into(),
        f.subject(),
        inputs,
        f.outputs(),
        c.clone(),
    )
    .unwrap();
    f.write(&document);
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::MetadataMismatch
    );
    document["metadata"]["input_digests"]["input2"] = json!(sha256(b"second snapshot"));
    f.write(&document);
    assert!(validate_json_table(&rule).unwrap().view().passed);
    let unpinned = vec![f.artifact("input", "dataset", None)];
    assert!(ApprovedJsonTableRule::from_verified_policy(
        "owner-1".into(),
        "policy-1".into(),
        f.subject(),
        unpinned,
        f.outputs(),
        c
    )
    .is_err());
}

#[test]
fn nested_approved_roots_cannot_alias_the_same_output_path() {
    let f = Fixture::new();
    let nested = f.root.join("nested");
    fs::create_dir(&nested).unwrap();
    let mut outputs = f.outputs();
    outputs[0] = f.artifact("table", "nested/shared", None);
    outputs[1] = BoundArtifact::from_approved_root(
        &nested,
        ArtifactRequirement {
            artifact_id: "script".into(),
            relative_path: "shared".into(),
            max_bytes: 1024,
            expected_sha256: None,
        },
    )
    .unwrap();
    assert!(ApprovedJsonTableRule::from_verified_policy(
        "owner-1".into(),
        "policy-1".into(),
        f.subject(),
        f.inputs(),
        outputs,
        f.contract()
    )
    .is_err());
}

#[test]
fn artifact_byte_limit_is_inclusive_and_never_bypassed_by_valid_json_whitespace() {
    let f = Fixture::new();
    let mut c = f.contract();
    let document = f.document(&c);
    let bytes = serde_json::to_vec(&document).unwrap();
    c.limits.max_bytes = bytes.len() as u64;
    let bind = |id: &str, path: &str, expected| {
        BoundArtifact::from_approved_root(
            &f.root,
            ArtifactRequirement {
                artifact_id: id.into(),
                relative_path: path.into(),
                max_bytes: c.limits.max_bytes,
                expected_sha256: expected,
            },
        )
        .unwrap()
    };
    let inputs = vec![bind(
        "input",
        "dataset",
        Some(sha256(b"fixed source snapshot\n")),
    )];
    let outputs = vec![
        bind("table", "table.json", None),
        bind("script", "reproduce.sh", None),
        bind("environment", "environment.json", None),
    ];
    let rule = ApprovedJsonTableRule::from_verified_policy(
        "owner-1".into(),
        "policy-1".into(),
        f.subject(),
        inputs,
        outputs,
        c,
    )
    .unwrap();
    f.raw(&bytes);
    assert!(validate_json_table(&rule).unwrap().view().passed);
    let mut oversized = bytes;
    oversized.push(b' ');
    f.raw(&oversized);
    let e = validate_json_table(&rule).unwrap();
    assert_eq!(code(&e), FailureCode::ArtifactIntegrity);
    assert_eq!(
        e.view()
            .file_integrity
            .outputs
            .iter()
            .find(|a| a.artifact_id == "table")
            .unwrap()
            .state,
        agentbaton_workspaces::validation::CheckState::TooLarge
    );
}

#[test]
fn symlinked_ancestor_and_nonregular_output_are_rejected_by_the_same_pinned_reader() {
    let f = Fixture::new();
    let c = f.contract();
    let nested = f.root.join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        nested.join("table.json"),
        serde_json::to_vec(&f.document(&c)).unwrap(),
    )
    .unwrap();
    let mut outputs = f.outputs();
    outputs[0] = f.artifact("table", "nested/table.json", None);
    let rule = ApprovedJsonTableRule::from_verified_policy(
        "owner-1".into(),
        "policy-1".into(),
        f.subject(),
        f.inputs(),
        outputs,
        c.clone(),
    )
    .unwrap();
    let e = validate_json_table(&rule).unwrap();
    assert!(e.view().passed);
    let moved = f.root.join("moved");
    fs::rename(&nested, &moved).unwrap();
    symlink(&moved, &nested).unwrap();
    assert!(e.verify_current().is_err());
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::ArtifactIntegrity
    );
    let ordinary = f.rule(c);
    let _socket = std::os::unix::net::UnixListener::bind(f.root.join("table.json")).unwrap();
    assert_eq!(
        code(&validate_json_table(&ordinary).unwrap()),
        FailureCode::ArtifactIntegrity
    );
}

#[test]
fn failed_receipts_are_stable_and_deep_nesting_stops_before_standard_parser_recursion_limit() {
    let f = Fixture::new();
    let c = f.contract();
    let rule = f.rule(c.clone());
    let mut bad = f.document(&c);
    bad["rows"][0]["x"] = json!(999);
    f.write(&bad);
    let first = validate_json_table(&rule).unwrap();
    let second = validate_json_table(&rule).unwrap();
    assert_eq!(first.view().digest, second.view().digest);
    assert_eq!(code(&first), FailureCode::NumericRange);
    let bytes = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    f.raw(bytes.as_bytes());
    assert_eq!(
        code(&validate_json_table(&rule).unwrap()),
        FailureCode::JsonDepthLimit
    );
}

#[test]
fn integer_negative_zero_decimal_and_exponent_spellings_are_conservatively_refused() {
    let f = Fixture::new();
    let c = f.contract();
    let rule = f.rule(c.clone());
    let document = serde_json::to_string(&f.document(&c)).unwrap();
    assert!(document.contains("\"n\":0"));
    for spelling in ["-0", "0.0", "0e0"] {
        f.raw(
            document
                .replacen("\"n\":0", &format!("\"n\":{spelling}"), 1)
                .as_bytes(),
        );
        let e = validate_json_table(&rule).unwrap();
        assert_eq!(code(&e), FailureCode::FieldType);
    }
}
