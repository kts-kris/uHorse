//! Web-based Skill Editor
//!
//! Provides a web UI for editing, validating, and managing skills.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod handlers;
mod templates;
mod validator;

pub use handlers::*;
pub use templates::*;
pub use validator::*;

/// Skill editor state
#[derive(Debug, Clone)]
pub struct SkillEditorState {
    pub skills_dir: std::path::PathBuf,
}

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// Skill content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContent {
    pub meta: SkillMeta,
    pub content: String,
    pub schema: Option<serde_json::Value>,
    pub validation_errors: Vec<ValidationError>,
}

/// Validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub severity: String, // "error", "warning", "info"
}

/// Skill editor router
pub fn skill_editor_router(state: Arc<SkillEditorState>) -> Router {
    Router::new()
        // UI routes
        .route("/skills/editor", get(skill_editor_ui))
        .route("/skills/editor/:name", get(skill_editor_ui_for))
        // API routes
        .route("/api/skills", get(list_skills_api))
        .route("/api/skills/:name", get(get_skill_api))
        .route("/api/skills/:name", put(update_skill_api))
        .route("/api/skills/:name", delete(delete_skill_api))
        .route("/api/skills", post(create_skill_api))
        .route("/api/skills/:name/validate", post(validate_skill_api))
        .route("/api/skills/templates", get(list_templates_api))
        .route("/api/skills/templates/:name", get(get_template_api))
        .with_state(state)
}

/// Skill editor UI
async fn skill_editor_ui() -> impl IntoResponse {
    Html(templates::skill_editor_html())
}

/// Skill editor UI for specific skill
async fn skill_editor_ui_for(
    Path(name): Path<String>,
) -> impl IntoResponse {
    Html(templates::skill_editor_html_with_skill(&name))
}

// API Handlers

/// List all skills
async fn list_skills_api(
    State(state): State<Arc<SkillEditorState>>,
) -> impl IntoResponse {
    match handlers::list_skills(&state.skills_dir) {
        Ok(skills) => Json(serde_json::json!({
            "success": true,
            "skills": skills
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// Get skill content
async fn get_skill_api(
    State(state): State<Arc<SkillEditorState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handlers::get_skill(&state.skills_dir, &name) {
        Ok(skill) => Json(serde_json::json!({
            "success": true,
            "skill": skill
        })).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// Update skill content
async fn update_skill_api(
    State(state): State<Arc<SkillEditorState>>,
    Path(name): Path<String>,
    Json(payload): Json<UpdateSkillPayload>,
) -> impl IntoResponse {
    match handlers::update_skill(&state.skills_dir, &name, &payload.content) {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "Skill updated successfully"
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// Create new skill
async fn create_skill_api(
    State(state): State<Arc<SkillEditorState>>,
    Json(payload): Json<CreateSkillPayload>,
) -> impl IntoResponse {
    match handlers::create_skill(&state.skills_dir, &payload.name, payload.template.as_deref()) {
        Ok(skill) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "success": true,
                "skill": skill
            }))
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// Delete skill
async fn delete_skill_api(
    State(state): State<Arc<SkillEditorState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handlers::delete_skill(&state.skills_dir, &name) {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "Skill deleted successfully"
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// Validate skill
async fn validate_skill_api(
    State(state): State<Arc<SkillEditorState>>,
    Path(name): Path<String>,
    Json(payload): Json<ValidatePayload>,
) -> impl IntoResponse {
    match validator::validate_skill_content(&payload.content) {
        Ok(errors) => Json(serde_json::json!({
            "success": true,
            "errors": errors,
            "valid": errors.iter().all(|e| e.severity != "error")
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

/// List skill templates
async fn list_templates_api() -> impl IntoResponse {
    let templates = handlers::list_templates();
    Json(serde_json::json!({
        "success": true,
        "templates": templates
    }))
}

/// Get skill template
async fn get_template_api(
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handlers::get_template(&name) {
        Some(template) => Json(serde_json::json!({
            "success": true,
            "template": template
        })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": "Template not found"
            }))
        ).into_response(),
    }
}

// Request payloads

#[derive(Debug, Deserialize)]
struct UpdateSkillPayload {
    content: String,
}

#[derive(Debug, Deserialize)]
struct CreateSkillPayload {
    name: String,
    template: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidatePayload {
    content: String,
}
