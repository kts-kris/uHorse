//! # Marketplace Handlers
//!
//! 技能市场端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;
use crate::store::Skill;

/// 模拟市场技能数据
fn get_mock_marketplace_skills() -> Vec<MarketplaceSkill> {
    vec![
        MarketplaceSkill {
            id: "marketplace/code-review".to_string(),
            name: "Code Review".to_string(),
            description: "自动代码审查技能，支持多种编程语言".to_string(),
            version: "1.2.0".to_string(),
            author: "uHorse Team".to_string(),
            downloads: 1520,
            rating: 4.8,
            tags: vec![
                "code".to_string(),
                "review".to_string(),
                "quality".to_string(),
            ],
            icon_url: None,
            repository_url: Some("https://github.com/uhorse/skill-code-review".to_string()),
        },
        MarketplaceSkill {
            id: "marketplace/translation".to_string(),
            name: "Translation".to_string(),
            description: "多语言翻译技能，支持 100+ 种语言".to_string(),
            version: "2.0.1".to_string(),
            author: "uHorse Team".to_string(),
            downloads: 3200,
            rating: 4.9,
            tags: vec!["translation".to_string(), "i18n".to_string()],
            icon_url: None,
            repository_url: Some("https://github.com/uhorse/skill-translation".to_string()),
        },
        MarketplaceSkill {
            id: "marketplace/summarization".to_string(),
            name: "Text Summarization".to_string(),
            description: "智能文本摘要技能，支持长文档压缩".to_string(),
            version: "1.5.0".to_string(),
            author: "Community".to_string(),
            downloads: 890,
            rating: 4.5,
            tags: vec!["nlp".to_string(), "summary".to_string()],
            icon_url: None,
            repository_url: None,
        },
        MarketplaceSkill {
            id: "marketplace/web-search".to_string(),
            name: "Web Search".to_string(),
            description: "网络搜索技能，支持多种搜索引擎".to_string(),
            version: "1.0.0".to_string(),
            author: "uHorse Team".to_string(),
            downloads: 2100,
            rating: 4.7,
            tags: vec!["search".to_string(), "web".to_string()],
            icon_url: None,
            repository_url: Some("https://github.com/uhorse/skill-web-search".to_string()),
        },
        MarketplaceSkill {
            id: "marketplace/data-analysis".to_string(),
            name: "Data Analysis".to_string(),
            description: "数据分析技能，支持 CSV、Excel、JSON 格式".to_string(),
            version: "1.1.0".to_string(),
            author: "Community".to_string(),
            downloads: 650,
            rating: 4.3,
            tags: vec![
                "data".to_string(),
                "analysis".to_string(),
                "visualization".to_string(),
            ],
            icon_url: None,
            repository_url: None,
        },
    ]
}

/// 搜索技能市场
#[axum::debug_handler]
pub async fn search_skills(
    State(_state): State<Arc<HttpState>>,
    Query(query): Query<MarketplaceSearchQuery>,
) -> impl IntoResponse {
    debug!("Searching marketplace: {:?}", query);

    let mut skills = get_mock_marketplace_skills();

    // 按关键词过滤
    if let Some(ref q) = query.q {
        let q_lower = q.to_lowercase();
        skills.retain(|s| {
            s.name.to_lowercase().contains(&q_lower)
                || s.description.to_lowercase().contains(&q_lower)
                || s.tags.iter().any(|t| t.to_lowercase().contains(&q_lower))
        });
    }

    // 按标签过滤
    if !query.tags.is_empty() {
        skills.retain(|s| {
            query
                .tags
                .iter()
                .all(|tag| s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
        });
    }

    // 排序
    match query.sort.as_deref() {
        Some("downloads") => skills.sort_by(|a, b| b.downloads.cmp(&a.downloads)),
        Some("rating") => skills.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap()),
        _ => {} // 默认按相关性排序（已过滤后的顺序）
    }

    (StatusCode::OK, Json(ApiResponse::success(skills)))
}

/// 获取市场技能详情
#[axum::debug_handler]
pub async fn get_marketplace_skill(
    State(_state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("Getting marketplace skill: {}", id);

    let skills = get_mock_marketplace_skills();
    match skills.into_iter().find(|s| s.id == id) {
        Some(skill) => (StatusCode::OK, Json(ApiResponse::success(skill))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<MarketplaceSkill>::error(
                "NOT_FOUND",
                "Skill not found in marketplace",
            )),
        ),
    }
}

/// 安装技能
#[axum::debug_handler]
pub async fn install_skill(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Json(req): Json<InstallSkillRequest>,
) -> impl IntoResponse {
    info!(
        "Installing skill from marketplace: {} to agent {:?}",
        id, req.agent_id
    );

    // 查找市场技能
    let skills = get_mock_marketplace_skills();
    let market_skill = match skills.into_iter().find(|s| s.id == id) {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<SkillDto>::error(
                    "NOT_FOUND",
                    "Skill not found in marketplace",
                )),
            );
        }
    };

    // 创建本地技能
    let skill = Skill::from_marketplace(market_skill, req.agent_id);
    let dto = skill.to_dto();
    state.store.create_skill(skill).await;

    (StatusCode::CREATED, Json(ApiResponse::success(dto)))
}
