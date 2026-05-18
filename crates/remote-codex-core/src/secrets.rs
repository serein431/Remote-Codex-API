use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use keyring::Entry;

use crate::{RemoteCodexError, Result};

const KEYRING_SERVICE: &str = "Remote Codex API";

pub trait SecretStore {
    fn set_token(&self, profile_id: &str, token: &str) -> Result<()>;
    fn get_token(&self, profile_id: &str) -> Result<Option<String>>;
    fn delete_token(&self, profile_id: &str) -> Result<()>;
}

#[derive(Clone, Default)]
pub struct InMemorySecretStore {
    values: Arc<Mutex<HashMap<String, String>>>,
}

impl SecretStore for InMemorySecretStore {
    fn set_token(&self, profile_id: &str, token: &str) -> Result<()> {
        self.values
            .lock()
            .map_err(|_| RemoteCodexError::Message("secret store lock poisoned".to_string()))?
            .insert(profile_id.to_string(), token.to_string());
        Ok(())
    }

    fn get_token(&self, profile_id: &str) -> Result<Option<String>> {
        Ok(self
            .values
            .lock()
            .map_err(|_| RemoteCodexError::Message("secret store lock poisoned".to_string()))?
            .get(profile_id)
            .cloned())
    }

    fn delete_token(&self, profile_id: &str) -> Result<()> {
        self.values
            .lock()
            .map_err(|_| RemoteCodexError::Message("secret store lock poisoned".to_string()))?
            .remove(profile_id);
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(profile_id: &str) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, profile_id)
            .map_err(|err| RemoteCodexError::Keyring(err.to_string()))
    }
}

impl SecretStore for KeyringSecretStore {
    fn set_token(&self, profile_id: &str, token: &str) -> Result<()> {
        Self::entry(profile_id)?
            .set_password(token)
            .map_err(|err| RemoteCodexError::Keyring(err.to_string()))
    }

    fn get_token(&self, profile_id: &str) -> Result<Option<String>> {
        match Self::entry(profile_id)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(RemoteCodexError::Keyring(err.to_string())),
        }
    }

    fn delete_token(&self, profile_id: &str) -> Result<()> {
        match Self::entry(profile_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(RemoteCodexError::Keyring(err.to_string())),
        }
    }
}
