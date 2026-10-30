//! Persist the optional QA / operator RPC token in the OS credential store
//! instead of webview localStorage (audit #61 item 61).

use keyring::{Entry, Error as KeyringError};

const SERVICE: &str = "network.boing.hub";
const ACCOUNT: &str = "operator-rpc-token";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_operator_token() -> Result<String, String> {
    match entry()?.get_password() {
        Ok(v) => Ok(v),
        Err(KeyringError::NoEntry) => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_operator_token(token: String) -> Result<(), String> {
    let e = entry()?;
    if token.is_empty() {
        match e.delete_credential() {
            Ok(()) => Ok(()),
            Err(KeyringError::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(&token).map_err(|err| err.to_string())
    }
}

#[tauri::command]
pub fn clear_operator_token() -> Result<(), String> {
    set_operator_token(String::new())
}
