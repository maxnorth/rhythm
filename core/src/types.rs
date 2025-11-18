use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ExecutionType {
    Task,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Suspended,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub id: String,
    #[serde(rename = "type")]
    pub exec_type: ExecutionType,
    pub function_name: String,
    pub queue: String,
    pub status: ExecutionStatus,

    pub args: JsonValue,
    pub kwargs: JsonValue,

    pub result: Option<JsonValue>,
    pub error: Option<JsonValue>,

    pub attempt: i32,
    pub max_retries: i32,

    pub parent_workflow_id: Option<String>,

    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExecutionParams {
    pub id: Option<String>,
    pub exec_type: ExecutionType,
    pub function_name: String,
    pub queue: String,
    pub args: JsonValue,
    pub kwargs: JsonValue,
    pub max_retries: i32,
    pub parent_workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionListFilter {
    pub queue: Option<String>,
    pub status: Option<ExecutionStatus>,
    pub limit: Option<i32>,
}
