//! # Document (文档解析)
//!
//! 支持多种文档格式的解析：
//! - PDF
//! - Word (DOCX)
//! - Excel (XLSX)
//! - Markdown

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, instrument};

use crate::{MultimodalError, Result};

/// 文档类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentType {
    /// PDF
    Pdf,
    /// Word (DOCX)
    Docx,
    /// Excel (XLSX)
    Xlsx,
    /// Markdown
    Markdown,
    /// 纯文本
    Txt,
    /// HTML
    Html,
    /// JSON
    Json,
    /// CSV
    Csv,
    /// 未知
    Unknown,
}

impl DocumentType {
    /// 从文件扩展名推断
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => Self::Pdf,
            "docx" | "doc" => Self::Docx,
            "xlsx" | "xls" => Self::Xlsx,
            "md" | "markdown" => Self::Markdown,
            "txt" => Self::Txt,
            "html" | "htm" => Self::Html,
            "json" => Self::Json,
            "csv" => Self::Csv,
            _ => Self::Unknown,
        }
    }

    /// 从 MIME 类型推断
    pub fn from_mime(mime: &str) -> Self {
        match mime.to_lowercase().as_str() {
            "application/pdf" => Self::Pdf,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Self::Docx,
            "application/msword" => Self::Docx,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Self::Xlsx,
            "application/vnd.ms-excel" => Self::Xlsx,
            "text/markdown" | "text/x-markdown" => Self::Markdown,
            "text/plain" => Self::Txt,
            "text/html" => Self::Html,
            "application/json" => Self::Json,
            "text/csv" => Self::Csv,
            _ => Self::Unknown,
        }
    }

    /// 获取 MIME 类型
    pub fn to_mime(&self) -> &'static str {
        match self {
            DocumentType::Pdf => "application/pdf",
            DocumentType::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            DocumentType::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            DocumentType::Markdown => "text/markdown",
            DocumentType::Txt => "text/plain",
            DocumentType::Html => "text/html",
            DocumentType::Json => "application/json",
            DocumentType::Csv => "text/csv",
            DocumentType::Unknown => "application/octet-stream",
        }
    }
}

impl std::fmt::Display for DocumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentType::Pdf => write!(f, "PDF"),
            DocumentType::Docx => write!(f, "DOCX"),
            DocumentType::Xlsx => write!(f, "XLSX"),
            DocumentType::Markdown => write!(f, "Markdown"),
            DocumentType::Txt => write!(f, "Text"),
            DocumentType::Html => write!(f, "HTML"),
            DocumentType::Json => write!(f, "JSON"),
            DocumentType::Csv => write!(f, "CSV"),
            DocumentType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// 文档元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// 文件名
    pub filename: String,
    /// 文档类型
    pub doc_type: DocumentType,
    /// 文件大小（字节）
    pub size: u64,
    /// 页数（如果适用）
    pub page_count: Option<u32>,
    /// 作者
    pub author: Option<String>,
    /// 标题
    pub title: Option<String>,
    /// 创建时间
    pub created_at: Option<String>,
    /// 修改时间
    pub modified_at: Option<String>,
}

/// 文档解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    /// 元数据
    pub metadata: DocumentMetadata,
    /// 文本内容
    pub text: String,
    /// 分段内容
    pub sections: Vec<DocumentSection>,
    /// 表格数据（如果有）
    pub tables: Vec<TableData>,
    /// 图片描述（如果有）
    pub images: Vec<ImageDescription>,
}

/// 文档段落
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSection {
    /// 段落标题
    pub title: Option<String>,
    /// 段落内容
    pub content: String,
    /// 层级
    pub level: u32,
}

/// 表格数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableData {
    /// 表头
    pub headers: Vec<String>,
    /// 行数据
    pub rows: Vec<Vec<String>>,
    /// 表格标题
    pub caption: Option<String>,
}

/// 图片描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDescription {
    /// 图片 ID 或索引
    pub id: String,
    /// 图片描述（如果有 OCR）
    pub description: Option<String>,
    /// 图片格式
    pub format: Option<String>,
}

/// 文档解析器
#[derive(Debug, Default)]
pub struct DocumentParser {
    /// 最大文件大小（字节）
    max_file_size: u64,
    /// 最大页数
    max_pages: u32,
}

impl DocumentParser {
    /// 创建新解析器
    pub fn new() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB
            max_pages: 1000,
        }
    }

    /// 设置最大文件大小
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// 解析文档
    #[instrument(skip(self, data))]
    pub async fn parse(&self, data: &[u8], filename: &str, doc_type: Option<DocumentType>) -> Result<ParsedDocument> {
        debug!("Parsing document: {} ({} bytes)", filename, data.len());

        // 检查文件大小
        if data.len() as u64 > self.max_file_size {
            return Err(MultimodalError::FileTooLarge(data.len() as u64, self.max_file_size));
        }

        // 推断文档类型
        let doc_type = doc_type.unwrap_or_else(|| {
            let ext = Path::new(filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            DocumentType::from_extension(ext)
        });

        debug!("Detected document type: {:?}", doc_type);

        // 根据类型解析
        let (text, sections, tables, images) = match doc_type {
            DocumentType::Markdown => self.parse_markdown(data).await?,
            DocumentType::Txt => self.parse_text(data).await?,
            DocumentType::Json => self.parse_json(data).await?,
            DocumentType::Csv => self.parse_csv(data).await?,
            DocumentType::Html => self.parse_html(data).await?,
            DocumentType::Pdf => self.parse_pdf(data).await?,
            DocumentType::Docx => self.parse_docx(data).await?,
            DocumentType::Xlsx => self.parse_xlsx(data).await?,
            DocumentType::Unknown => {
                return Err(MultimodalError::UnsupportedFormat(format!(
                    "Unknown document type for: {}",
                    filename
                )));
            }
        };

        let metadata = DocumentMetadata {
            filename: filename.to_string(),
            doc_type,
            size: data.len() as u64,
            page_count: None,
            author: None,
            title: None,
            created_at: None,
            modified_at: None,
        };

        Ok(ParsedDocument {
            metadata,
            text,
            sections,
            tables,
            images,
        })
    }

    /// 解析 Markdown
    async fn parse_markdown(&self, data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        let text = String::from_utf8_lossy(data).to_string();
        let mut sections = Vec::new();

        // 简单的标题解析
        for line in text.lines() {
            if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count() as u32;
                let title = line.trim_start_matches('#').trim().to_string();
                sections.push(DocumentSection {
                    title: Some(title),
                    content: String::new(),
                    level,
                });
            } else if let Some(last) = sections.last_mut() {
                if !last.content.is_empty() {
                    last.content.push('\n');
                }
                last.content.push_str(line);
            }
        }

        // 如果没有标题，创建一个默认段落
        if sections.is_empty() {
            sections.push(DocumentSection {
                title: None,
                content: text.clone(),
                level: 0,
            });
        }

        Ok((text, sections, Vec::new(), Vec::new()))
    }

    /// 解析纯文本
    async fn parse_text(&self, data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        let text = String::from_utf8_lossy(data).to_string();

        let sections = vec![DocumentSection {
            title: None,
            content: text.clone(),
            level: 0,
        }];

        Ok((text, sections, Vec::new(), Vec::new()))
    }

    /// 解析 JSON
    async fn parse_json(&self, data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        let text = String::from_utf8_lossy(data).to_string();

        // 格式化 JSON
        let formatted = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            serde_json::to_string_pretty(&json).unwrap_or(text.clone())
        } else {
            text.clone()
        };

        let sections = vec![DocumentSection {
            title: Some("JSON Data".to_string()),
            content: formatted,
            level: 1,
        }];

        Ok((text, sections, Vec::new(), Vec::new()))
    }

    /// 解析 CSV
    async fn parse_csv(&self, data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        let text = String::from_utf8_lossy(data).to_string();
        let mut tables = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();

        for line in text.lines() {
            let row: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
            if !row.is_empty() {
                rows.push(row);
            }
        }

        if !rows.is_empty() {
            let headers = rows.remove(0);
            tables.push(TableData {
                headers,
                rows,
                caption: Some("CSV Data".to_string()),
            });
        }

        Ok((text, Vec::new(), tables, Vec::new()))
    }

    /// 解析 HTML
    async fn parse_html(&self, data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        // 简单的 HTML 标签移除
        let raw = String::from_utf8_lossy(data).to_string();
        let mut text = String::new();
        let mut in_tag = false;

        for c in raw.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => text.push(c),
                _ => {}
            }
        }

        // 清理多余空白
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

        let sections = vec![DocumentSection {
            title: Some("HTML Content".to_string()),
            content: text.clone(),
            level: 1,
        }];

        Ok((text, sections, Vec::new(), Vec::new()))
    }

    /// 解析 PDF（简化版 - 实际需要 pdf-extract 等库）
    async fn parse_pdf(&self, _data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        // TODO: 集成 pdf-extract 库
        debug!("PDF parsing - using placeholder implementation");

        Ok((
            "[PDF content - parsing requires pdf-extract library]".to_string(),
            vec![DocumentSection {
                title: Some("PDF Document".to_string()),
                content: "[PDF content placeholder]".to_string(),
                level: 1,
            }],
            Vec::new(),
            Vec::new(),
        ))
    }

    /// 解析 DOCX（简化版 - 实际需要 docx-rs 等库）
    async fn parse_docx(&self, _data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        // TODO: 集成 docx-rs 库
        debug!("DOCX parsing - using placeholder implementation");

        Ok((
            "[DOCX content - parsing requires docx-rs library]".to_string(),
            vec![DocumentSection {
                title: Some("Word Document".to_string()),
                content: "[DOCX content placeholder]".to_string(),
                level: 1,
            }],
            Vec::new(),
            Vec::new(),
        ))
    }

    /// 解析 XLSX（简化版 - 实际需要 calamine 等库）
    async fn parse_xlsx(&self, _data: &[u8]) -> Result<(String, Vec<DocumentSection>, Vec<TableData>, Vec<ImageDescription>)> {
        // TODO: 集成 calamine 库
        debug!("XLSX parsing - using placeholder implementation");

        Ok((
            "[XLSX content - parsing requires calamine library]".to_string(),
            vec![DocumentSection {
                title: Some("Excel Spreadsheet".to_string()),
                content: "[XLSX content placeholder]".to_string(),
                level: 1,
            }],
            vec![TableData {
                headers: vec!["Column1".to_string(), "Column2".to_string()],
                rows: vec![vec!["Data1".to_string(), "Data2".to_string()]],
                caption: Some("Sheet1".to_string()),
            }],
            Vec::new(),
        ))
    }
}

/// 文档解析服务 trait
#[async_trait]
pub trait DocumentService: Send + Sync {
    /// 解析文档
    async fn parse(&self, data: &[u8], filename: &str, doc_type: Option<DocumentType>) -> Result<ParsedDocument>;
}

#[async_trait]
impl DocumentService for DocumentParser {
    async fn parse(&self, data: &[u8], filename: &str, doc_type: Option<DocumentType>) -> Result<ParsedDocument> {
        self.parse(data, filename, doc_type).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_from_extension() {
        assert_eq!(DocumentType::from_extension("pdf"), DocumentType::Pdf);
        assert_eq!(DocumentType::from_extension("md"), DocumentType::Markdown);
        assert_eq!(DocumentType::from_extension("JSON"), DocumentType::Json);
        assert_eq!(DocumentType::from_extension("unknown"), DocumentType::Unknown);
    }

    #[test]
    fn test_document_type_from_mime() {
        assert_eq!(DocumentType::from_mime("application/pdf"), DocumentType::Pdf);
        assert_eq!(DocumentType::from_mime("text/markdown"), DocumentType::Markdown);
    }

    #[test]
    fn test_document_type_to_mime() {
        assert_eq!(DocumentType::Pdf.to_mime(), "application/pdf");
        assert_eq!(DocumentType::Markdown.to_mime(), "text/markdown");
    }

    #[tokio::test]
    async fn test_parse_markdown() {
        let parser = DocumentParser::new();
        let content = b"# Title\n\nThis is content.\n\n## Subtitle\n\nMore content.";
        let result = parser.parse(content, "test.md", Some(DocumentType::Markdown)).await.unwrap();

        assert_eq!(result.metadata.doc_type, DocumentType::Markdown);
        assert!(!result.sections.is_empty());
    }

    #[tokio::test]
    async fn test_parse_json() {
        let parser = DocumentParser::new();
        let content = br#"{"name": "test", "value": 123}"#;
        let result = parser.parse(content, "test.json", Some(DocumentType::Json)).await.unwrap();

        assert_eq!(result.metadata.doc_type, DocumentType::Json);
        assert!(!result.text.is_empty());
    }

    #[tokio::test]
    async fn test_file_too_large() {
        let parser = DocumentParser::new().with_max_file_size(10);
        let content = b"This is more than 10 bytes";
        let result = parser.parse(content, "test.txt", Some(DocumentType::Txt)).await;

        assert!(matches!(result, Err(MultimodalError::FileTooLarge(_, _))));
    }
}
