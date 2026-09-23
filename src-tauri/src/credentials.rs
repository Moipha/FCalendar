use keyring::Entry;

const SERVICE: &str = "com.f.cal";

pub fn credential_ref_for_account(account_id: &str) -> String {
    // Windows 凭据名避免 `/`，旧版 `account/{id}` 读取时再回退。
    format!("account-{account_id}")
}

fn legacy_credential_ref(account_id: &str) -> String {
    format!("account/{account_id}")
}

pub fn store_password(credential_ref: &str, password: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, credential_ref).map_err(|e| format!("凭据条目: {e}"))?;
    entry
        .set_password(password)
        .map_err(|e| format!("保存密码: {e}"))
}

pub fn load_password(credential_ref: &str) -> Result<String, String> {
    let entry = Entry::new(SERVICE, credential_ref).map_err(|e| format!("凭据条目: {e}"))?;
    match entry.get_password() {
        Ok(p) => Ok(p),
        Err(_) => {
            if let Some(id) = credential_ref.strip_prefix("account-") {
                let legacy = Entry::new(SERVICE, &legacy_credential_ref(id))
                    .map_err(|e| format!("凭据条目: {e}"))?;
                legacy.get_password().map_err(|e| format!("读取密码: {e}"))
            } else {
                Err("读取密码失败".into())
            }
        }
    }
}

pub fn delete_password(credential_ref: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, credential_ref).map_err(|e| format!("凭据条目: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除密码: {e}")),
    }
}
