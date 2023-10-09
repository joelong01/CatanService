use ::serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AllocateResourceForwardData {
    pub user_id: String,
    pub is_last: bool,
}
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AllocateResourceReverseData {
    pub user_id: String,
    pub is_first: bool,
}
