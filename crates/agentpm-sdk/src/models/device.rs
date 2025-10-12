use serde::Serialize;

#[derive(Serialize)]
pub struct DeviceStartReq {
    pub scopes: Vec<String>,
    pub client: String,
    pub device_meta: serde_json::Value,
}

#[derive(Serialize)]
pub struct DevicePollReq {
    pub device_code: String,
}
