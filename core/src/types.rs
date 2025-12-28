//! V2 Type Definitions
//!
//! Core types for the V2 workflow engine.

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
    pub target_name: String,
    pub queue: String,
    pub status: ExecutionStatus,

    pub inputs: JsonValue,
    pub output: Option<JsonValue>,

    pub attempt: i32,

    pub parent_workflow_id: Option<String>,

    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExecutionParams {
    pub id: Option<String>,
    pub exec_type: ExecutionType,
    pub target_name: String,
    pub queue: String,
    pub inputs: JsonValue,
    pub parent_workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleExecutionParams {
    pub exec_type: ExecutionType,
    pub target_name: String,
    pub queue: String,
    pub inputs: JsonValue,
    pub run_at: chrono::NaiveDateTime,
}

/// Filters for querying executions
#[derive(Default, Debug, Clone)]
pub struct ExecutionFilters {
    /// Filter by parent workflow ID (to get child tasks)
    pub parent_workflow_id: Option<String>,

    /// Filter by execution status
    pub status: Option<ExecutionStatus>,

    /// Filter by function/workflow name
    pub target_name: Option<String>,

    /// Limit number of results
    pub limit: Option<i64>,

    /// Offset for pagination
    pub offset: Option<i64>,
}

/// Outcome of an execution (success, failure, or suspended)
#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    Success(JsonValue),
    Failure(JsonValue),
    Suspended,
}

/// A signal sent to a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub workflow_id: String,
    pub signal_name: String,
    pub payload: JsonValue,
    pub created_at: DateTime<Utc>,
}
