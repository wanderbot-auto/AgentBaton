//! Fixed Host methods called only after the existing owner/API/request guards.
//! No startup, actor priority, scoped capability, contact or retry entry point.
use super::{invalid, Request};
use crate::registry::{HostCreationReceipt, HostListRequest};
use crate::storage::Store;
use crate::{CoreError, Result};
use agentbaton_remote_host::registration::HostRegistration;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

const MAX_HOST_PARAMS: usize = 16 * 1024;
const MAX_HOST_RESPONSE: usize = 512 * 1024;

struct LimitedJson {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bounded JSON limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn bounded_json<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> std::result::Result<Vec<u8>, ()> {
    let mut writer = LimitedJson {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| ())?;
    Ok(writer.bytes)
}
fn decode_params<T: DeserializeOwned>(request: &Request) -> Result<T> {
    // The existing envelope already owns a Value. This is its serialized-byte
    // bound before typed decoding, not preservation of original raw spans or
    // duplicate keys erased by that earlier parser.
    let bytes = bounded_json(&request.params, MAX_HOST_PARAMS).map_err(|_| invalid())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
fn read_envelope(request: &Request) -> Result<()> {
    if request.expected_revision.is_some() || request.idempotency_key.is_some() {
        return Err(invalid());
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindHostRegistration {
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetHostRegistration {
    host_id: String,
}
#[derive(Serialize)]
struct HostRegistrationLookup {
    lookup_version: u32,
    key: String,
    receipt: Option<HostCreationReceipt>,
}
#[derive(Serialize)]
struct HostResponseBound<'a> {
    request_id: &'a str,
    result: &'a Value,
    revision: u64,
    warnings: [(); 0],
}
fn response_error(committed: bool) -> CoreError {
    if committed {
        let mut error = CoreError::new(
            "OUTCOME_UNKNOWN",
            "Host registration response unavailable; reconcile the original creation key",
        );
        error.side_effect_state = "unknown".into();
        error
    } else {
        CoreError::new(
            "RESOURCE_LIMIT",
            "Host response exceeds its bounded response contract",
        )
    }
}
fn response<T: Serialize>(request: &Request, result: &T, committed: bool) -> Result<Value> {
    let value = serde_json::to_value(result).map_err(|_| response_error(committed))?;
    // Match handle_observed's fixed envelope, including exact request-ID JSON
    // escaping, empty warnings and the maximum possible global u64 revision.
    // This dominates its actual compact serialized length without reading time
    // or mistaking the outer global sequence for Host revision/created_sequence.
    let envelope = HostResponseBound {
        request_id: &request.request_id,
        result: &value,
        revision: u64::MAX,
        warnings: [],
    };
    bounded_json(&envelope, MAX_HOST_RESPONSE).map_err(|_| response_error(committed))?;
    Ok(value)
}
pub(super) fn register(store: &mut Store, request: &Request) -> Result<Value> {
    if request.expected_revision.is_some() {
        return Err(invalid());
    }
    let key = request.idempotency_key.as_deref().ok_or_else(invalid)?;
    let registration: HostRegistration = decode_params(request)?;
    // Store invokes the reviewed pure validator and immutable transaction. Its
    // error/side-effect state passes through unchanged; no lookup/retry here.
    let receipt = store.register_host(&registration, key)?;
    response(request, &receipt, true)
}
pub(super) fn find_registration(store: &mut Store, request: &Request) -> Result<Value> {
    read_envelope(request)?;
    let query: FindHostRegistration = decode_params(request)?;
    let receipt = store.find_host_registration(&query.key)?;
    response(
        request,
        &HostRegistrationLookup {
            lookup_version: 1,
            key: query.key,
            receipt,
        },
        false,
    )
}
pub(super) fn get(store: &mut Store, request: &Request) -> Result<Value> {
    read_envelope(request)?;
    let query: GetHostRegistration = decode_params(request)?;
    let view = store.get_host_registration(&query.host_id)?;
    response(request, &view, false)
}
pub(super) fn list(store: &mut Store, request: &Request) -> Result<Value> {
    read_envelope(request)?;
    let query: HostListRequest = decode_params(request)?;
    let page = store.list_host_registrations(&query)?;
    response(request, &page, false)
}

#[cfg(test)]
#[path = "host_registry_ipc_tests.rs"]
mod tests;
