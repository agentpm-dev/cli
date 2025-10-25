use serde::Serialize;

#[derive(Serialize)]
pub struct CreateNamespaceSignerReq {
    pub label: String,
    pub algo: String,
    pub public_key_b64: String,
}

#[derive(Serialize)]
pub struct RevokeNamespaceSignerReq {
    pub is_active: bool,
}
