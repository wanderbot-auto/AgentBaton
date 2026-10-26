//! Fixed JSON-table contract checks, not scientific correctness, script execution,
//! task acceptance or integration authority. Only trusted host policy code may
//! construct ApprovedJsonTableRule after authenticating its owner approval.
use crate::validation::{
    sha256, valid_id, validate_artifacts, AnalysisReproducibility, AnalysisScope, ApprovedRule,
    BoundArtifact, ValidationError, ValidationEvidence, ValidationPolicyContractView,
    ValidationSubject, ValidationView, VersionBinding,
};
use serde::{
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserialize, Serialize,
};
use serde_json::{json, Map, Number, Value};
use std::{collections::BTreeSet, fmt};

pub const VALIDATOR_ID: &str = "builtin.data-json-table.v1";
const MAX_TOTAL_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonLimits {
    pub max_bytes: u64,
    pub max_depth: u32,
    /// Counts every value and every object key, including metadata.
    pub max_values: u32,
    pub max_string_bytes: u32,
    pub max_rows: u32,
}
impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_bytes: 1_048_576,
            max_depth: 16,
            max_values: 100_000,
            max_string_bytes: 4096,
            max_rows: 10_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScalarType {
    Boolean,
    String {
        max_bytes: u32,
    },
    /// Signed i64 integer representations, never rounded through f64.
    /// Decimal/exponent spellings and serde_json negative-zero floats are refused.
    Integer {
        min: i64,
        max: i64,
    },
    /// Finite IEEE-754 binary64 semantics, not arbitrary-precision decimal math.
    Number {
        min: f64,
        max: f64,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub required: bool,
    pub nullable: bool,
    pub scalar: ScalarType,
}
/// Serializable proposal only. Deserializing this DTO does not authorize reading
/// artifacts or selecting these checks. Unknown columns are always rejected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonTableContract {
    pub table_artifact_id: String,
    pub columns: Vec<Column>,
    pub dataset_rows: u64,
    pub analysis: AnalysisReproducibility,
    pub limits: JsonLimits,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JsonTablePolicyView {
    pub validator_id: String,
    /// This is the separate integrity layer, not the numerical check result.
    pub artifact_policy: ValidationPolicyContractView,
    pub table: JsonTableContract,
}
impl JsonTablePolicyView {
    pub fn digest(&self) -> Result<String, ValidationError> {
        serde_json::to_vec(self)
            .map(|bytes| sha256(&bytes))
            .map_err(|_| ValidationError::InvalidContract)
    }
}

/// Not Deserialize. This is an internal host-policy boundary, not authentication
/// of arbitrary approval strings; never expose this constructor as a renderer API.
///
/// ```compile_fail
/// use agentbaton_workspaces::deterministic::ApprovedJsonTableRule;
/// let forged = serde_json::from_str::<ApprovedJsonTableRule>("{}");
/// ```
#[derive(Clone)]
pub struct ApprovedJsonTableRule {
    approval_id: String,
    policy_revision: String,
    policy: JsonTablePolicyView,
    contract_digest: String,
    integrity: ApprovedRule,
    inputs: Vec<BoundArtifact>,
    outputs: Vec<BoundArtifact>,
}
impl ApprovedJsonTableRule {
    pub fn from_verified_policy(
        approval_id: String,
        policy_revision: String,
        subject: ValidationSubject,
        inputs: Vec<BoundArtifact>,
        outputs: Vec<BoundArtifact>,
        mut table: JsonTableContract,
    ) -> Result<Self, ValidationError> {
        let VersionBinding::Data { .. } = &subject.version else {
            return Err(ValidationError::InvalidContract);
        };
        let limits = &table.limits;
        // Bound proposal work before sorting/cloning attacker-influenced strings.
        if !valid_id(&approval_id)
            || !valid_id(&policy_revision)
            || !valid_id(&table.table_artifact_id)
            || !valid_id(&table.analysis.script_artifact_id)
            || !valid_id(&table.analysis.environment_artifact_id)
            || !valid_id(&table.analysis.timezone)
            || table.analysis.units.is_empty()
            || table.analysis.units.len() > 64
            || table
                .analysis
                .units
                .iter()
                .any(|(key, unit)| !column_name(key) || unit.is_empty() || unit.len() > 128)
            || matches!(&table.analysis.scope, AnalysisScope::Sample { rows, limitation } if *rows == 0 || limitation.is_empty() || limitation.len() > 4096)
            || table
                .columns
                .iter()
                .take(65)
                .any(|column| !column_name(&column.name))
        {
            return Err(ValidationError::InvalidContract);
        }
        if inputs.is_empty()
            || inputs.len() > 16
            || outputs.len() != 3
            || table.columns.is_empty()
            || table.columns.len() > 64
            || !(1..=4_194_304).contains(&limits.max_bytes)
            || !(1..=32).contains(&limits.max_depth)
            || !(1..=200_000).contains(&limits.max_values)
            || !(1..=16_384).contains(&limits.max_string_bytes)
            || !(1..=20_000).contains(&limits.max_rows)
        {
            return Err(ValidationError::InvalidContract);
        }
        table.columns.sort_by(|a, b| a.name.cmp(&b.name));
        let mut names = BTreeSet::new();
        let mut numeric = BTreeSet::new();
        for column in &table.columns {
            if !column_name(&column.name) || !names.insert(column.name.clone()) {
                return Err(ValidationError::InvalidContract);
            }
            match column.scalar {
                ScalarType::Boolean => (),
                ScalarType::String { max_bytes } => {
                    if max_bytes == 0 || max_bytes > limits.max_string_bytes {
                        return Err(ValidationError::InvalidContract);
                    }
                }
                ScalarType::Integer { min, max } => {
                    if min > max {
                        return Err(ValidationError::InvalidContract);
                    }
                    numeric.insert(column.name.clone());
                }
                ScalarType::Number { min, max } => {
                    if !min.is_finite() || !max.is_finite() || min > max {
                        return Err(ValidationError::InvalidContract);
                    }
                    numeric.insert(column.name.clone());
                }
            }
        }
        if table
            .analysis
            .units
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            != numeric
        {
            return Err(ValidationError::InvalidContract);
        }
        let expected_rows = match &table.analysis.scope {
            AnalysisScope::Full => table.dataset_rows,
            AnalysisScope::Sample { rows, .. } if *rows <= table.dataset_rows => *rows,
            _ => return Err(ValidationError::InvalidContract),
        };
        if expected_rows > u64::from(limits.max_rows) {
            return Err(ValidationError::InvalidContract);
        }
        let mut total = 0u64;
        let mut locations = BTreeSet::new();
        let mut all_ids = BTreeSet::new();
        for artifact in inputs.iter().chain(&outputs) {
            let view = artifact.contract_view();
            total = total
                .checked_add(view.requirement.max_bytes)
                .ok_or(ValidationError::InvalidContract)?;
            if view.requirement.max_bytes > limits.max_bytes
                || !locations.insert(view.canonical_root.join(&view.requirement.relative_path))
                || !all_ids.insert(view.requirement.artifact_id)
            {
                return Err(ValidationError::InvalidContract);
            }
        }
        if total > MAX_TOTAL_ARTIFACT_BYTES
            || table.table_artifact_id == table.analysis.script_artifact_id
            || table.table_artifact_id == table.analysis.environment_artifact_id
            || !outputs
                .iter()
                .any(|a| a.contract_view().requirement.artifact_id == table.table_artifact_id)
        {
            return Err(ValidationError::InvalidContract);
        }
        let integrity = ApprovedRule::from_verified_policy(
            approval_id.clone(),
            policy_revision.clone(),
            subject,
            &inputs,
            &outputs,
            Some(table.analysis.clone()),
        )?;
        let policy = JsonTablePolicyView {
            validator_id: VALIDATOR_ID.into(),
            artifact_policy: integrity.contract_view(),
            table,
        };
        let contract_digest = policy.digest()?;
        Ok(Self {
            approval_id,
            policy_revision,
            policy,
            contract_digest,
            integrity,
            inputs,
            outputs,
        })
    }
    pub fn contract_view(&self) -> JsonTablePolicyView {
        self.policy.clone()
    }
    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }
}
fn column_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    ArtifactIntegrity,
    ArtifactChanged,
    JsonSyntax,
    JsonDuplicateKey,
    JsonDepthLimit,
    JsonValueLimit,
    JsonStringLimit,
    RootShape,
    MetadataMismatch,
    RowsShape,
    RowCount,
    MissingField,
    UnexpectedField,
    FieldType,
    NumericRange,
    StringLength,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CheckFailure {
    pub code: FailureCode,
    /// A fixed JSON pointer or approved column name; never an observed value.
    pub pointer: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}
fn failure(code: FailureCode, pointer: impl Into<String>) -> CheckFailure {
    CheckFailure {
        code,
        pointer: pointer.into(),
        line: None,
        column: None,
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct JsonTableValidationView {
    pub validator_id: String,
    pub approval_id: String,
    pub policy_revision: String,
    pub contract: JsonTablePolicyView,
    pub contract_digest: String,
    pub file_integrity: ValidationView,
    pub table_sha256: Option<String>,
    pub checked_rows: Option<u64>,
    pub failure: Option<CheckFailure>,
    pub passed: bool,
    pub digest: String,
}
/// Private receipt; editing or deserializing its view cannot produce a token.
pub struct JsonTableEvidence {
    view: JsonTableValidationView,
    files: ValidationEvidence,
}
impl JsonTableEvidence {
    pub fn view(&self) -> JsonTableValidationView {
        self.view.clone()
    }
    pub fn verify_current(&self) -> Result<(), ValidationError> {
        self.files.verify_current()
    }
    pub fn accept_contract(
        &self,
        rule: &ApprovedJsonTableRule,
    ) -> Result<JsonTableContractAcceptance, ValidationError> {
        if self.view.contract_digest != rule.contract_digest
            || self.view.approval_id != rule.approval_id
            || self.view.policy_revision != rule.policy_revision
        {
            return Err(ValidationError::ApprovalMismatch);
        }
        self.verify_current()?;
        if !self.view.passed {
            return Err(ValidationError::ValidationFailed);
        }
        Ok(JsonTableContractAcceptance {
            validator_id: VALIDATOR_ID.into(),
            validation_digest: self.view.digest.clone(),
            contract_digest: rule.contract_digest.clone(),
            approval_id: rule.approval_id.clone(),
            policy_revision: rule.policy_revision.clone(),
            subject: rule.policy.artifact_policy.subject.clone(),
        })
    }
}
/// Accepts this data contract only, never algorithms, a whole task, Git adoption,
/// deployment or a script's successful execution. Not Deserialize.
///
/// ```compile_fail
/// use agentbaton_workspaces::{deterministic::JsonTableContractAcceptance, validation::Acceptance};
/// fn elevate(value: JsonTableContractAcceptance) -> Acceptance { value }
/// ```
#[derive(Clone, Debug, Serialize)]
pub struct JsonTableContractAcceptance {
    validator_id: String,
    validation_digest: String,
    contract_digest: String,
    approval_id: String,
    policy_revision: String,
    subject: ValidationSubject,
}
impl JsonTableContractAcceptance {
    pub fn validation_digest(&self) -> &str {
        &self.validation_digest
    }
    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }
    pub fn subject(&self) -> &ValidationSubject {
        &self.subject
    }
}

pub fn validate_json_table(
    rule: &ApprovedJsonTableRule,
) -> Result<JsonTableEvidence, ValidationError> {
    let files = validate_artifacts(
        &rule.integrity,
        rule.inputs.clone(),
        rule.outputs.clone(),
        Some(rule.policy.table.analysis.clone()),
    )?;
    let file_view = files.view();
    let mut view = JsonTableValidationView {
        validator_id: VALIDATOR_ID.into(),
        approval_id: rule.approval_id.clone(),
        policy_revision: rule.policy_revision.clone(),
        contract: rule.policy.clone(),
        contract_digest: rule.contract_digest.clone(),
        file_integrity: file_view.clone(),
        table_sha256: None,
        checked_rows: None,
        failure: None,
        passed: false,
        digest: String::new(),
    };
    if !file_view.passed {
        view.failure = Some(failure(FailureCode::ArtifactIntegrity, ""));
    } else {
        let table = rule
            .outputs
            .iter()
            .find(|a| {
                a.contract_view().requirement.artifact_id == rule.policy.table.table_artifact_id
            })
            .expect("validated table binding");
        match table.read() {
            Err(_) => view.failure = Some(failure(FailureCode::ArtifactChanged, "")),
            Ok(bytes) => {
                let hash = sha256(&bytes);
                let observed = file_view
                    .outputs
                    .iter()
                    .find(|a| a.artifact_id == rule.policy.table.table_artifact_id)
                    .and_then(|a| a.observed_sha256.as_ref());
                view.table_sha256 = Some(hash.clone());
                if observed != Some(&hash) {
                    view.failure = Some(failure(FailureCode::ArtifactChanged, ""));
                } else {
                    match parse_bounded(&bytes, &rule.policy.table.limits) {
                        Err(error) => view.failure = Some(error),
                        Ok(value) => match check_table(&value, rule, &file_view) {
                            Ok(rows) => view.checked_rows = Some(rows),
                            Err(error) => view.failure = Some(error),
                        },
                    }
                }
            }
        }
    }
    if files.verify_current().is_err() {
        view.failure = Some(failure(FailureCode::ArtifactChanged, ""));
    }
    view.passed = view.failure.is_none();
    // Domain-separated receipt; no timestamps, random IDs, raw data or mutable
    // acceptance booleans are included outside these fixed observed checks.
    view.digest = sha256(&serde_json::to_vec(&view).expect("serializable evidence"));
    Ok(JsonTableEvidence { view, files })
}

fn check_table(
    value: &Value,
    rule: &ApprovedJsonTableRule,
    files: &ValidationView,
) -> Result<u64, CheckFailure> {
    let table = &rule.policy.table;
    let object = value
        .as_object()
        .ok_or_else(|| failure(FailureCode::RootShape, ""))?;
    if object.len() != 2 || !object.contains_key("metadata") || !object.contains_key("rows") {
        return Err(failure(FailureCode::RootShape, ""));
    }
    let rows = object["rows"]
        .as_array()
        .ok_or_else(|| failure(FailureCode::RowsShape, "/rows"))?;
    let expected_rows = match table.analysis.scope {
        AnalysisScope::Full => table.dataset_rows,
        AnalysisScope::Sample { rows, .. } => rows,
    };
    if rows.len() as u64 != expected_rows || rows.len() > table.limits.max_rows as usize {
        return Err(failure(FailureCode::RowCount, "/rows"));
    }
    let dataset = match &rule.policy.artifact_policy.subject.version {
        VersionBinding::Data { dataset_revision } => dataset_revision,
        _ => unreachable!("data-only policy"),
    };
    let input_digests: std::collections::BTreeMap<_, _> = files
        .inputs
        .iter()
        .map(|a| (a.artifact_id.clone(), a.observed_sha256.clone()))
        .collect();
    let output_hash = |id: &str| {
        files
            .outputs
            .iter()
            .find(|a| a.artifact_id == id)
            .and_then(|a| a.observed_sha256.as_deref())
            .expect("all integrity checks passed")
    };
    let expected_metadata = json!({
        "input_revision": rule.policy.artifact_policy.subject.input_revision,
        "dataset_revision": dataset,
        "input_digests": input_digests,
        "table_artifact_id": table.table_artifact_id,
        "row_count": expected_rows,
        "dataset_rows": table.dataset_rows,
        "units": table.analysis.units,
        "timezone": table.analysis.timezone,
        "seed": table.analysis.seed,
        "scope": table.analysis.scope,
        "script_sha256": output_hash(&table.analysis.script_artifact_id),
        "environment_sha256": output_hash(&table.analysis.environment_artifact_id),
    });
    let metadata = object["metadata"]
        .as_object()
        .ok_or_else(|| failure(FailureCode::MetadataMismatch, "/metadata"))?;
    let expected = expected_metadata.as_object().unwrap();
    if metadata.len() != expected.len() {
        return Err(failure(FailureCode::MetadataMismatch, "/metadata"));
    }
    for (key, required) in expected {
        if metadata.get(key) != Some(required) {
            return Err(failure(
                FailureCode::MetadataMismatch,
                format!("/metadata/{key}"),
            ));
        }
    }
    for (index, row) in rows.iter().enumerate() {
        let row = row
            .as_object()
            .ok_or_else(|| failure(FailureCode::RowsShape, format!("/rows/{index}")))?;
        if row
            .keys()
            .any(|key| !table.columns.iter().any(|column| &column.name == key))
        {
            return Err(failure(
                FailureCode::UnexpectedField,
                format!("/rows/{index}"),
            ));
        }
        for column in &table.columns {
            let pointer = format!("/rows/{index}/{}", column.name);
            let Some(value) = row.get(&column.name) else {
                if column.required {
                    return Err(failure(FailureCode::MissingField, pointer));
                }
                continue;
            };
            if value.is_null() {
                if column.nullable {
                    continue;
                }
                return Err(failure(FailureCode::FieldType, pointer));
            }
            let valid = match column.scalar {
                ScalarType::Boolean => value.is_boolean(),
                ScalarType::String { max_bytes } => {
                    let Some(text) = value.as_str() else {
                        return Err(failure(FailureCode::FieldType, pointer));
                    };
                    if text.len() > max_bytes as usize {
                        return Err(failure(FailureCode::StringLength, pointer));
                    }
                    true
                }
                ScalarType::Integer { min, max } => {
                    let Some(number) = value.as_i64() else {
                        return Err(failure(FailureCode::FieldType, pointer));
                    };
                    if number < min || number > max {
                        return Err(failure(FailureCode::NumericRange, pointer));
                    }
                    true
                }
                ScalarType::Number { min, max } => {
                    let Some(number) = value.as_f64() else {
                        return Err(failure(FailureCode::FieldType, pointer));
                    };
                    if !number.is_finite() || number < min || number > max {
                        return Err(failure(FailureCode::NumericRange, pointer));
                    }
                    true
                }
            };
            if !valid {
                return Err(failure(FailureCode::FieldType, pointer));
            }
        }
    }
    Ok(rows.len() as u64)
}

// serde_json performs lexical/Unicode/number parsing. This visitor adds bounded
// allocation traversal and duplicate-key rejection; it is not a new JSON parser.
struct Budget<'a> {
    limits: &'a JsonLimits,
    values: u32,
    violation: Option<FailureCode>,
}
impl Budget<'_> {
    fn reject<E: serde::de::Error>(&mut self, code: FailureCode) -> E {
        self.violation = Some(code);
        E::custom("bounded JSON contract rejected")
    }
    fn consume<E: serde::de::Error>(&mut self) -> Result<(), E> {
        if self.values >= self.limits.max_values {
            return Err(self.reject(FailureCode::JsonValueLimit));
        }
        self.values += 1;
        Ok(())
    }
    fn string<E: serde::de::Error>(&mut self, value: &str) -> Result<(), E> {
        if value.len() > self.limits.max_string_bytes as usize {
            return Err(self.reject(FailureCode::JsonStringLimit));
        }
        Ok(())
    }
}
struct Seed<'a, 'b> {
    budget: &'a mut Budget<'b>,
    depth: u32,
}
impl<'de> DeserializeSeed<'de> for Seed<'_, '_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.depth > self.budget.limits.max_depth {
            return Err(self.budget.reject(FailureCode::JsonDepthLimit));
        }
        self.budget.consume()?;
        deserializer.deserialize_any(BoundedVisitor {
            budget: self.budget,
            depth: self.depth,
        })
    }
}
struct BoundedVisitor<'a, 'b> {
    budget: &'a mut Budget<'b>,
    depth: u32,
}
impl<'de> Visitor<'de> for BoundedVisitor<'_, '_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON data")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        self.budget.string(v)?;
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        self.budget.string(&v)?;
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut array = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            array.push(value);
        }
        Ok(Value::Array(array))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget.consume()?;
            self.budget.string(&key)?;
            if object.contains_key(&key) {
                return Err(self.budget.reject(FailureCode::JsonDuplicateKey));
            }
            let value = map.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
fn parse_bounded(bytes: &[u8], limits: &JsonLimits) -> Result<Value, CheckFailure> {
    let mut budget = Budget {
        limits,
        values: 0,
        violation: None,
    };
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = Seed {
        budget: &mut budget,
        depth: 1,
    }
    .deserialize(&mut deserializer)
    .and_then(|value| {
        deserializer.end()?;
        Ok(value)
    });
    result.map_err(|error| CheckFailure {
        code: budget.violation.unwrap_or(FailureCode::JsonSyntax),
        pointer: String::new(),
        line: Some(error.line()),
        column: Some(error.column()),
    })
}
