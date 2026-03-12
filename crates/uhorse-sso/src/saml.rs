//! SAML 2.0 Client
//!
//! 实现 SAML 2.0 企业 SSO 集成

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// SAML 配置
#[derive(Debug, Clone)]
pub struct SamlConfig {
    /// IdP 元数据 URL
    pub idp_metadata_url: String,
    /// IdP 实体 ID
    pub idp_entity_id: String,
    /// IdP SSO URL
    pub idp_sso_url: String,
    /// IdP SLO URL (可选)
    pub idp_slo_url: Option<String>,
    /// SP 实体 ID
    pub sp_entity_id: String,
    /// SP ACS URL
    pub sp_acs_url: String,
    /// SP SLO URL (可选)
    pub sp_slo_url: Option<String>,
    /// X.509 证书
    pub idp_certificate: String,
    /// SP 私钥
    pub sp_private_key: Option<String>,
    /// SP 证书
    pub sp_certificate: Option<String>,
    /// NameID 格式
    pub name_id_format: String,
    /// 签名算法
    pub signature_algorithm: String,
    /// 摘要算法
    pub digest_algorithm: String,
}

impl Default for SamlConfig {
    fn default() -> Self {
        Self {
            idp_metadata_url: String::new(),
            idp_entity_id: String::new(),
            idp_sso_url: String::new(),
            idp_slo_url: None,
            sp_entity_id: String::new(),
            sp_acs_url: String::new(),
            sp_slo_url: None,
            idp_certificate: String::new(),
            sp_private_key: None,
            sp_certificate: None,
            name_id_format: "urn:oasis:names:tc:SAML:2.0:nameid-format:transient".to_string(),
            signature_algorithm: "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256".to_string(),
            digest_algorithm: "http://www.w3.org/2001/04/xmlenc#sha256".to_string(),
        }
    }
}

/// SAML 断言
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlAssertion {
    /// ID
    pub id: String,
    /// Issue Instant
    pub issue_instant: DateTime<Utc>,
    /// Issuer
    pub issuer: String,
    /// Subject
    pub subject: SamlSubject,
    /// Conditions
    pub conditions: Option<SamlConditions>,
    /// 属性
    pub attributes: HashMap<String, Vec<String>>,
    /// 认证上下文
    pub authn_context: Option<SamlAuthnContext>,
}

/// SAML 主题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlSubject {
    /// NameID
    pub name_id: String,
    /// NameID 格式
    pub name_id_format: String,
    /// 确认数据
    pub confirmation: Option<SamlSubjectConfirmation>,
}

/// SAML 主题确认
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlSubjectConfirmation {
    /// 方法
    pub method: String,
    /// NotOnOrAfter
    pub not_on_or_after: DateTime<Utc>,
    /// Recipient
    pub recipient: String,
}

/// SAML 条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlConditions {
    /// NotBefore
    pub not_before: DateTime<Utc>,
    /// NotOnOrAfter
    pub not_on_or_after: DateTime<Utc>,
    /// 受众限制
    pub audience_restrictions: Vec<String>,
}

/// SAML 认证上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlAuthnContext {
    /// 认证上下文类引用
    pub authn_context_class_ref: String,
    /// 认证时间
    pub authn_instant: DateTime<Utc>,
    /// 会话索引
    pub session_index: Option<String>,
}

/// SAML 响应
#[derive(Debug, Clone)]
pub struct SamlResponse {
    /// 原始 XML
    pub raw_xml: String,
    /// 断言
    pub assertion: SamlAssertion,
    /// 签名有效
    pub signature_valid: bool,
}

/// SAML 客户端
pub struct SamlClient {
    /// 配置
    config: SamlConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl SamlClient {
    /// 创建新的 SAML 客户端
    pub fn new(config: SamlConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// 生成 SAML 认证请求
    pub fn generate_auth_request(&self, relay_state: Option<&str>) -> crate::Result<String> {
        let id = format!("id{}", uuid::Uuid::new_v4().simple());
        let issue_instant = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");

        let auth_request = format!(
            r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
    ID="{}"
    Version="2.0"
    IssueInstant="{}"
    ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
    AssertionConsumerServiceURL="{}">
    <saml:Issuer xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">{}</saml:Issuer>
    <samlp:NameIDPolicy xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
        Format="{}"
        AllowCreate="true"/>
</samlp:AuthnRequest>"#,
            id,
            issue_instant,
            self.config.sp_acs_url,
            self.config.sp_entity_id,
            self.config.name_id_format,
        );

        // Base64 编码
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            auth_request.as_bytes(),
        );

        // 构建 SSO URL
        let sso_url = if let Some(state) = relay_state {
            format!(
                "{}?SAMLRequest={}&RelayState={}",
                self.config.idp_sso_url,
                urlencoding::encode(&encoded),
                urlencoding::encode(state)
            )
        } else {
            format!(
                "{}?SAMLRequest={}",
                self.config.idp_sso_url,
                urlencoding::encode(&encoded)
            )
        };

        Ok(sso_url)
    }

    /// 解析 SAML 响应
    pub fn parse_response(&self, saml_response: &str) -> crate::Result<SamlResponse> {
        // Base64 解码
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            saml_response,
        ).map_err(|e| crate::SsoError::SamlError(format!("Failed to decode SAML response: {}", e)))?;

        let raw_xml = String::from_utf8(decoded)
            .map_err(|e| crate::SsoError::SamlError(format!("Invalid UTF-8 in SAML response: {}", e)))?;

        // 简化解析：提取关键信息
        let assertion = self.parse_assertion(&raw_xml)?;

        Ok(SamlResponse {
            raw_xml,
            assertion,
            signature_valid: false, // 实际实现需要验证签名
        })
    }

    /// 解析断言
    fn parse_assertion(&self, xml: &str) -> crate::Result<SamlAssertion> {
        // 简化实现：使用字符串解析
        // 实际实现应使用 XML 解析器如 roxmltree

        let id = extract_attribute(xml, "ID").unwrap_or_default();
        let issuer = extract_element_content(xml, "saml:Issuer").unwrap_or_default();

        let name_id = extract_element_content(xml, "saml:NameID").unwrap_or_default();
        let name_id_format = extract_attribute(xml, "Format").unwrap_or_default();

        let subject = SamlSubject {
            name_id,
            name_id_format,
            confirmation: None,
        };

        // 提取属性
        let mut attributes = HashMap::new();
        if let Some(attrs_start) = xml.find("<saml:AttributeStatement>") {
            if let Some(attrs_end) = xml.find("</saml:AttributeStatement>") {
                let attrs_xml = &xml[attrs_start..attrs_end];
                // 简化：提取常见属性
                if let Some(email) = extract_element_content(attrs_xml, "saml:AttributeValue") {
                    attributes.insert("email".to_string(), vec![email]);
                }
            }
        }

        Ok(SamlAssertion {
            id,
            issue_instant: Utc::now(),
            issuer,
            subject,
            conditions: None,
            attributes,
            authn_context: None,
        })
    }

    /// 生成 SAML 注销请求
    pub fn generate_logout_request(&self, name_id: &str, session_index: Option<&str>) -> crate::Result<String> {
        let id = format!("id{}", uuid::Uuid::new_v4().simple());
        let issue_instant = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");

        let session_index_xml = session_index
            .map(|s| format!("<samlp:SessionIndex>{}</samlp:SessionIndex>", s))
            .unwrap_or_default();

        let logout_request = format!(
            r#"<samlp:LogoutRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
    ID="{}"
    Version="2.0"
    IssueInstant="{}">
    <saml:Issuer xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">{}</saml:Issuer>
    <saml:NameID xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
        Format="{}">{}</saml:NameID>
    {}
</samlp:LogoutRequest>"#,
            id,
            issue_instant,
            self.config.sp_entity_id,
            self.config.name_id_format,
            name_id,
            session_index_xml,
        );

        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            logout_request.as_bytes(),
        );

        Ok(encoded)
    }

    /// 获取 SP 元数据
    pub fn generate_sp_metadata(&self) -> String {
        format!(
            r#"<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata"
    entityID="{}">
    <md:SPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
        <md:NameIDFormat>{}</md:NameIDFormat>
        <md:AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
            Location="{}"
            index="1"/>
    </md:SPSSODescriptor>
</md:EntityDescriptor>"#,
            self.config.sp_entity_id,
            self.config.name_id_format,
            self.config.sp_acs_url,
        )
    }

    /// 从 IdP 元数据加载配置
    pub async fn load_idp_metadata(&mut self, metadata_url: &str) -> crate::Result<()> {
        info!("Loading IdP metadata from: {}", metadata_url);

        let response = self
            .http_client
            .get(metadata_url)
            .send()
            .await
            .map_err(|e| crate::SsoError::SamlError(format!("Failed to fetch IdP metadata: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::SsoError::SamlError(format!(
                "Failed to fetch IdP metadata: HTTP {}",
                response.status()
            )));
        }

        let metadata = response
            .text()
            .await
            .map_err(|e| crate::SsoError::SamlError(format!("Failed to read IdP metadata: {}", e)))?;

        // 简化解析：提取关键信息
        self.config.idp_entity_id = extract_element_content(&metadata, "md:EntityID")
            .or_else(|| extract_attribute(&metadata, "entityID"))
            .unwrap_or_default();

        self.config.idp_sso_url = extract_element_content(&metadata, "md:Location")
            .or_else(|| extract_attribute(&metadata, "Location"))
            .unwrap_or_default();

        // 提取证书
        if let Some(cert) = extract_element_content(&metadata, "ds:X509Certificate") {
            self.config.idp_certificate = cert;
        }

        Ok(())
    }
}

/// 从 XML 提取元素内容
fn extract_element_content(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    if let Some(start) = xml.find(&start_tag) {
        let content_start = start + start_tag.len();
        if let Some(end) = xml[content_start..].find(&end_tag) {
            return Some(xml[content_start..content_start + end].trim().to_string());
        }
    }
    None
}

/// 从 XML 提取属性
fn extract_attribute(xml: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = xml.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = xml[value_start..].find('"') {
            return Some(xml[value_start..value_start + end].to_string());
        }
    }
    None
}

/// URL 编码模块
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saml_config_default() {
        let config = SamlConfig::default();
        assert_eq!(config.name_id_format, "urn:oasis:names:tc:SAML:2.0:nameid-format:transient");
        assert!(config.idp_slo_url.is_none());
    }

    #[test]
    fn test_generate_auth_request() {
        let config = SamlConfig {
            idp_sso_url: "https://idp.example.com/sso".to_string(),
            sp_entity_id: "https://sp.example.com".to_string(),
            sp_acs_url: "https://sp.example.com/acs".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:2.0:nameid-format:transient".to_string(),
            ..Default::default()
        };

        let client = SamlClient::new(config);
        let url = client.generate_auth_request(Some("state123")).unwrap();

        assert!(url.contains("SAMLRequest="));
        assert!(url.contains("RelayState=state123"));
    }

    #[test]
    fn test_generate_sp_metadata() {
        let config = SamlConfig {
            sp_entity_id: "https://sp.example.com".to_string(),
            sp_acs_url: "https://sp.example.com/acs".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:2.0:nameid-format:transient".to_string(),
            ..Default::default()
        };

        let client = SamlClient::new(config);
        let metadata = client.generate_sp_metadata();

        assert!(metadata.contains("entityID=\"https://sp.example.com\""));
        assert!(metadata.contains("Location=\"https://sp.example.com/acs\""));
    }

    #[test]
    fn test_extract_element_content() {
        let xml = r#"<root><name>Test User</name></root>"#;
        let content = extract_element_content(xml, "name");
        assert_eq!(content, Some("Test User".to_string()));
    }

    #[test]
    fn test_extract_attribute() {
        let xml = r#"<element id="123" name="test"/>"#;
        let attr = extract_attribute(xml, "id");
        assert_eq!(attr, Some("123".to_string()));
    }
}
