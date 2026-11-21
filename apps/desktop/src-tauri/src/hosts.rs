//! Fixed Host inventory and registration recovery commands for the desktop Runtime.
use super::Runtime;
use agentbaton_core::{
    client_recovery_journal::{
        ClientRecoveryJournal, HostRegistration, HostRegistrationIntent, HostRegistrationLookup,
    },
    ipc,
    registry::{HostCreationReceipt, HostListRequest},
    CoreError, Result as CoreResult,
};
use serde_json::{json, Value};
use std::{
    io::{self, Write},
    path::Path,
};
use tauri::ipc::{InvokeBody, Request};

const MAX_PARAMS: usize = 16 * 1024;
const MAX_ARGUMENTS: usize = MAX_PARAMS + 256;
const MAX_CORE_RESPONSE: usize = 512 * 1024;
const MAX_CREATED_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy)]
enum Operation {
    Register,
    List,
    Get,
    Pending,
    Reconcile,
    Retry,
    Acknowledge,
}

impl Operation {
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Register => &["registration", "idempotencyKey"],
            Self::List => &["request"],
            Self::Get => &["hostId"],
            Self::Pending => &[],
            Self::Reconcile | Self::Retry => &["entryId"],
            Self::Acknowledge => &["entryId", "hostId", "createdSequence"],
        }
    }
}

struct Failure {
    error: CoreError,
    response: Option<Box<Value>>,
}

type BridgeResult<T> = Result<T, Failure>;

impl From<CoreError> for Failure {
    fn from(error: CoreError) -> Self {
        Self {
            error,
            response: None,
        }
    }
}

impl Failure {
    fn into_response(self) -> Value {
        match self.response {
            Some(response) => *response,
            None => json!({"request_id":null,"error":self.error,"revision":0,"warnings":[]}),
        }
    }
}

fn local_result(result: Value) -> Value {
    json!({"request_id":null,"result":result,"revision":0,"warnings":[]})
}

fn invalid_request() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Malformed or oversized Host command arguments",
    )
}

fn invalid_response() -> CoreError {
    CoreError::new(
        "INVALID_CORE_RESPONSE",
        "Malformed Host response from the local daemon",
    )
}

fn unknown_submission(intent: &HostRegistrationIntent) -> CoreError {
    let mut error = CoreError::new(
        "OUTCOME_UNKNOWN",
        "Host registration could have committed; reconcile the original pending request",
    );
    error.side_effect_state = "unknown".into();
    error.details_ref = Some(format!("host-registration-recovery:{}", intent.entry_id));
    error
}

fn exact_fields(value: &Value, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == fields.len() && fields.iter().all(|key| object.contains_key(*key))
    })
}

// Bound the serialized Value at the Tauri/IPC boundary. Core still owns its
// transport-frame and typed document bounds.
struct Remaining(usize);

impl Write for Remaining {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("Host JSON byte limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fits(value: &Value, limit: usize) -> bool {
    serde_json::to_writer(Remaining(limit), value).is_ok()
}

fn checked_arguments(operation: Operation, args: &Value) -> CoreResult<Value> {
    if !exact_fields(args, operation.fields()) || !fits(args, MAX_ARGUMENTS) {
        return Err(invalid_request());
    }
    Ok(args.clone())
}

fn uuid(value: &Value) -> Option<&str> {
    value.as_str().filter(|id| {
        id.len() == 36
            && id.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                }
            })
    })
}

fn call_core(
    core: &mut impl FnMut(Value) -> CoreResult<Value>,
    command: Value,
    write: Option<&HostRegistrationIntent>,
) -> BridgeResult<Value> {
    let malformed = || write.map_or_else(invalid_response, unknown_submission);
    // The current Core client uses one request per authenticated connection.
    // Its internally generated request UUID is not exposed for Host comparisons.
    let reply = core(command).map_err(|error| {
        // A framing error can occur after a Host create commits.
        write.map_or(error, unknown_submission)
    })?;
    let outcome = if reply.get("error").is_some() {
        "error"
    } else {
        "result"
    };
    if !exact_fields(&reply, &["request_id", outcome, "revision", "warnings"])
        || uuid(&reply["request_id"]).is_none()
        || reply["revision"].as_u64().is_none()
        || !reply["warnings"].is_array()
        || !fits(&reply, MAX_CORE_RESPONSE)
    {
        return Err(malformed().into());
    }
    if outcome == "error" {
        let error: CoreError =
            serde_json::from_value(reply["error"].clone()).map_err(|_| malformed())?;
        if !matches!(
            error.side_effect_state.as_str(),
            "none" | "not_started" | "confirmed" | "unknown"
        ) || reply["error"].get("details_ref").is_none()
        {
            return Err(malformed().into());
        }
        return Err(Failure {
            error,
            response: Some(Box::new(reply)),
        });
    }
    if !reply["result"].is_object() {
        return Err(malformed().into());
    }
    Ok(reply)
}

fn find_registration(
    core: &mut impl FnMut(Value) -> CoreResult<Value>,
    key: &str,
) -> BridgeResult<(Value, HostRegistrationLookup)> {
    let mut reply = call_core(
        core,
        json!({"method":"hosts.find_registration","params":{"key":key}}),
        None,
    )?;
    let lookup = serde_json::from_value(reply["result"].take()).map_err(|_| invalid_response())?;
    Ok((reply, lookup))
}

fn submission(
    mut reply: Value,
    intent: &HostRegistrationIntent,
    receipt: HostCreationReceipt,
) -> Value {
    reply["result"] = json!({"submission_version":1,"entry_id":intent.entry_id,"receipt":receipt});
    reply
}

fn submit(
    core: &mut impl FnMut(Value) -> CoreResult<Value>,
    intent: &HostRegistrationIntent,
) -> BridgeResult<Value> {
    let mut reply = call_core(
        core,
        json!({
            "method":"hosts.register",
            "params":intent.registration,
            "idempotency_key":intent.request_key,
        }),
        Some(intent),
    )?;
    let receipt =
        serde_json::from_value(reply["result"].take()).map_err(|_| unknown_submission(intent))?;
    // A create receipt comes from the single authenticated write. The same Core
    // validation used for lookup checks its semantics and captured metadata.
    let checked = HostRegistrationLookup {
        lookup_version: 1,
        key: intent.request_key.clone(),
        receipt: Some(receipt),
    };
    intent
        .validate_lookup(&checked)
        .map_err(|_| unknown_submission(intent))?;
    let receipt = checked.receipt.ok_or_else(|| unknown_submission(intent))?;
    Ok(submission(reply, intent, receipt))
}

fn stale_intent() -> CoreError {
    CoreError::new(
        "CLIENT_RECOVERY_REQUIRED",
        "The selected Host registration is no longer pending; reload the private journal",
    )
}

fn execute(
    operation: Operation,
    mut args: Value,
    runtime: &Path,
    mut core: impl FnMut(Value) -> CoreResult<Value>,
) -> BridgeResult<Value> {
    match operation {
        Operation::Register => {
            let key = uuid(&args["idempotencyKey"])
                .ok_or_else(invalid_request)?
                .to_owned();
            if !fits(&args["registration"], MAX_PARAMS) {
                return Err(invalid_request().into());
            }
            let registration: HostRegistration =
                serde_json::from_value(args["registration"].take())
                    .map_err(|_| invalid_request())?;
            let journal = ClientRecoveryJournal::new(runtime)?;
            let intent = journal.reserve_host_registration(key, registration)?;
            submit(&mut core, &intent)
        }
        Operation::List => {
            if !fits(&args["request"], MAX_PARAMS) {
                return Err(invalid_request().into());
            }
            let request: HostListRequest =
                serde_json::from_value(args["request"].take()).map_err(|_| invalid_request())?;
            call_core(
                &mut core,
                json!({"method":"hosts.list","params":request}),
                None,
            )
        }
        Operation::Get => {
            let host_id = uuid(&args["hostId"]).ok_or_else(invalid_request)?;
            call_core(
                &mut core,
                json!({"method":"hosts.get","params":{"host_id":host_id}}),
                None,
            )
        }
        Operation::Pending => {
            let intent = ClientRecoveryJournal::new(runtime)?.pending_host_registration()?;
            Ok(local_result(json!({"pending_version":1,"intent":intent})))
        }
        Operation::Reconcile | Operation::Retry => {
            let entry_id = uuid(&args["entryId"]).ok_or_else(invalid_request)?;
            let journal = ClientRecoveryJournal::new(runtime)?;
            let intent = journal
                .pending_host_registration()?
                .filter(|intent| intent.entry_id == entry_id)
                .ok_or_else(stale_intent)?;
            let (mut reply, lookup) = find_registration(&mut core, &intent.request_key)?;
            intent.validate_lookup(&lookup)?;
            if matches!(operation, Operation::Reconcile) {
                reply["result"] =
                    json!({"recovery_version":1,"entry_id":intent.entry_id,"lookup":lookup});
                return Ok(reply);
            }
            if let Some(receipt) = lookup.receipt {
                return Ok(submission(reply, &intent, receipt));
            }
            if journal.pending_host_registration()?.as_ref() != Some(&intent) {
                return Err(stale_intent().into());
            }
            // A racing acknowledgement requires an already-committed receipt.
            // The same key and body remain idempotent if that commit now exists.
            submit(&mut core, &intent)
        }
        Operation::Acknowledge => {
            let entry_id = uuid(&args["entryId"]).ok_or_else(invalid_request)?;
            let host_id = uuid(&args["hostId"]).ok_or_else(invalid_request)?;
            let sequence = args["createdSequence"]
                .as_u64()
                .filter(|seq| (1..=MAX_CREATED_SEQUENCE).contains(seq))
                .ok_or_else(invalid_request)?;
            let journal = ClientRecoveryJournal::new(runtime)?;
            let mut lookup_failure = None;
            let acknowledged =
                journal.acknowledge_host_registration(entry_id, host_id, sequence, |key| {
                    match find_registration(&mut core, key) {
                        Ok((_, lookup)) => Ok(lookup),
                        Err(failure) => {
                            let error = failure.error.clone();
                            lookup_failure = Some(failure);
                            Err(error)
                        }
                    }
                });
            if let Some(failure) = lookup_failure {
                return Err(failure);
            }
            Ok(local_result(json!({
                "acknowledgement_version":1,
                "entry_id":entry_id,
                "acknowledged":acknowledged?,
            })))
        }
    }
}

async fn invoke(
    operation: Operation,
    request: Request<'_>,
    runtime: &Runtime,
) -> Result<Value, String> {
    let InvokeBody::Json(args) = request.body() else {
        return Ok(Failure::from(invalid_request()).into_response());
    };
    let args = match checked_arguments(operation, args) {
        Ok(args) => args,
        Err(error) => return Ok(Failure::from(error).into_response()),
    };
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        execute(operation, args, &directory, |command| {
            ipc::call(&directory, command)
        })
        .unwrap_or_else(Failure::into_response)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn hosts_register(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Register, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_list(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::List, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_get(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Get, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_registration_pending(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Pending, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_registration_reconcile(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Reconcile, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_registration_retry(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Retry, request, &runtime).await
}

#[tauri::command]
pub(super) async fn hosts_registration_acknowledge(
    request: Request<'_>,
    runtime: tauri::State<'_, Runtime>,
) -> Result<Value, String> {
    invoke(Operation::Acknowledge, request, &runtime).await
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
