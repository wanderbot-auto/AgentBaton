use super::{err, valid_timestamp, valid_uuid, ClientRecoveryJournal, Result};
use crate::{registry::HostCreationReceipt, CoreError};
use agentbaton_remote_host::protocol::digest;
pub use agentbaton_remote_host::registration::HostRegistration;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const HOST_LOCK: &str = "host-registration-recovery.lock";
const HOST_JOURNAL: &str = "host-registration-recovery.json";
const INTENT_VERSION: u32 = 1;
const LOOKUP_VERSION: u32 = 1;

/// The original request captured for local recovery. Its fields confer no authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostRegistrationIntent {
    pub intent_version: u32,
    pub entry_id: String,
    pub request_key: String,
    pub registration: HostRegistration,
    pub registration_sha256: String,
    pub created_at: String,
}

/// Result of the fixed read-only lookup, including its echoed request key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostRegistrationLookup {
    pub lookup_version: u32,
    pub key: String,
    // The protocol distinguishes explicit absence from a malformed reply.
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub receipt: Option<HostCreationReceipt>,
}

fn registration_conflict() -> CoreError {
    CoreError::new(
        "CONFIG_CONFLICT",
        "Host registration receipt does not match the original request or acknowledgement",
    )
}

fn registration_digest(registration: &HostRegistration) -> Option<String> {
    registration.validate().ok()?;
    let bytes = serde_json::to_vec(registration).ok()?;
    Some(digest(&bytes))
}

impl HostRegistrationIntent {
    /// Verifies a lookup against the captured intent without changing the journal.
    pub fn validate_lookup(&self, lookup: &HostRegistrationLookup) -> Result<()> {
        self.validate()?;
        if lookup.lookup_version != LOOKUP_VERSION {
            return Err(CoreError::new(
                "VERSION_UNSUPPORTED",
                "Host registration lookup version is unsupported",
            ));
        }
        if lookup.key != self.request_key {
            return Err(registration_conflict());
        }
        if let Some(receipt) = &lookup.receipt {
            receipt.validate()?;
            if receipt.registration != self.registration {
                return Err(registration_conflict());
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.intent_version != INTENT_VERSION
            || !valid_uuid(&self.entry_id)
            || !valid_uuid(&self.request_key)
            || !valid_timestamp(&self.created_at)
            || registration_digest(&self.registration).as_deref()
                != Some(self.registration_sha256.as_str())
        {
            return Err(err());
        }
        Ok(())
    }
}

impl ClientRecoveryJournal {
    // Call while holding HOST_LOCK.
    fn read_host_registration(&self) -> Result<Option<HostRegistrationIntent>> {
        let mut entries: Vec<HostRegistrationIntent> = self.read_entries(HOST_JOURNAL)?;
        if entries.len() > 1 {
            return Err(err());
        }
        let pending = entries.pop();
        if let Some(intent) = &pending {
            intent.validate()?;
        }
        Ok(pending)
    }

    /// Durably captures one request before dispatch. An occupied journal preserves the
    /// original request, including when a caller attempts to reserve the same key again.
    pub fn reserve_host_registration(
        &self,
        request_key: String,
        registration: HostRegistration,
    ) -> Result<HostRegistrationIntent> {
        let invalid = || CoreError::new("INVALID_REQUEST", "Invalid Host registration request");
        if !valid_uuid(&request_key) {
            return Err(invalid());
        }
        let registration_sha256 = registration_digest(&registration).ok_or_else(invalid)?;
        let _lock = self.lock_file(HOST_LOCK)?;
        if self.read_host_registration()?.is_some() {
            return Err(CoreError::new(
                "CLIENT_RECOVERY_REQUIRED",
                "Check the previous Host registration before submitting another request",
            ));
        }
        let created_at = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .map_err(|_| err())?
            .format(&Rfc3339)
            .map_err(|_| err())?;
        if !valid_timestamp(&created_at) {
            return Err(err());
        }
        let intent = HostRegistrationIntent {
            intent_version: INTENT_VERSION,
            entry_id: Uuid::new_v4().to_string(),
            request_key,
            registration,
            registration_sha256,
            created_at,
        };
        self.write_entries(HOST_JOURNAL, std::slice::from_ref(&intent))?;
        Ok(intent)
    }

    /// Reads the exact captured request without connecting to the daemon or the Host.
    pub fn pending_host_registration(&self) -> Result<Option<HostRegistrationIntent>> {
        let _lock = self.lock_file(HOST_LOCK)?;
        self.read_host_registration()
    }

    /// Clears the exact current intent after a fresh, matching registration lookup.
    /// The trusted native caller supplies the fixed read-only lookup, outside the journal lock.
    /// Returns false for a missing receipt or a missing, stale, or replaced intent.
    pub fn acknowledge_host_registration(
        &self,
        entry_id: &str,
        host_id: &str,
        created_sequence: u64,
        lookup: impl FnOnce(&str) -> Result<HostRegistrationLookup>,
    ) -> Result<bool> {
        let Some(intent) = self
            .pending_host_registration()?
            .filter(|i| i.entry_id == entry_id)
        else {
            return Ok(false);
        };
        let found = lookup(&intent.request_key)?;
        intent.validate_lookup(&found)?;
        let Some(receipt) = found.receipt else {
            return Ok(false);
        };
        if receipt.host_id != host_id || receipt.created_sequence != created_sequence {
            return Err(registration_conflict());
        }

        let _lock = self.lock_file(HOST_LOCK)?;
        if self.read_host_registration()?.as_ref() != Some(&intent) {
            return Ok(false);
        }
        self.write_entries::<HostRegistrationIntent>(HOST_JOURNAL, &[])
            .map_err(|mut error| {
                if error.side_effect_state == "unknown" {
                    error.message = "Host registration acknowledgement outcome is unknown; read the pending registration again".into();
                }
                error
            })?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "host_registration_recovery_tests.rs"]
mod tests;
