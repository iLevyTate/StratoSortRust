// API Documentation Generation
// Automatic OpenAPI/Swagger specification generation with interactive documentation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AppError;
use crate::api::versioning::{ApiVersion, ApiEndpoint, HttpMethod};

// OpenAPI specification structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: ApiInfo,
    pub servers: Vec<ApiServer>,
    pub paths: HashMap<String, PathItem>,
    pub components: Components,
    pub tags: Vec<Tag>,
    pub external_docs: Option<ExternalDocs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInfo {
    pub title: String,
    pub description: String,
    pub version: String,
    pub terms_of_service: Option<String>,
    pub contact: Option<Contact>,
    pub license: Option<License>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiServer {
    pub url: String,
    pub description: String,
    pub variables: HashMap<String, ServerVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerVariable {
    pub default: String,
    pub description: Option<String>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub get: Option<Operation>,
    pub put: Option<Operation>,
    pub post: Option<Operation>,
    pub delete: Option<Operation>,
    pub options: Option<Operation>,
    pub head: Option<Operation>,
    pub patch: Option<Operation>,
    pub trace: Option<Operation>,
    pub servers: Option<Vec<ApiServer>>,
    pub parameters: Option<Vec<Parameter>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub tags: Vec<String>,
    pub summary: String,
    pub description: Option<String>,
    pub external_docs: Option<ExternalDocs>,
    pub operation_id: String,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: HashMap<String, Response>,
    pub callbacks: Option<HashMap<String, Callback>>,
    pub deprecated: bool,
    pub security: Option<Vec<SecurityRequirement>>,
    pub servers: Option<Vec<ApiServer>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub in_location: ParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub deprecated: bool,
    pub allow_empty_value: bool,
    pub schema: Option<Schema>,
    pub example: Option<serde_json::Value>,
    pub examples: Option<HashMap<String, Example>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    Query,
    Header,
    Path,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    pub description: Option<String>,
    pub content: HashMap<String, MediaType>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    pub headers: Option<HashMap<String, Header>>,
    pub content: Option<HashMap<String, MediaType>>,
    pub links: Option<HashMap<String, Link>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: Option<Schema>,
    pub example: Option<serde_json::Value>,
    pub examples: Option<HashMap<String, Example>>,
    pub encoding: Option<HashMap<String, Encoding>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub format: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub default: Option<serde_json::Value>,
    pub nullable: bool,
    pub discriminator: Option<Discriminator>,
    pub read_only: bool,
    pub write_only: bool,
    pub xml: Option<Xml>,
    pub external_docs: Option<ExternalDocs>,
    pub example: Option<serde_json::Value>,
    pub deprecated: bool,
    pub properties: Option<HashMap<String, Box<Schema>>>,
    pub required: Option<Vec<String>>,
    pub items: Option<Box<Schema>>,
    pub all_of: Option<Vec<Box<Schema>>>,
    pub one_of: Option<Vec<Box<Schema>>>,
    pub any_of: Option<Vec<Box<Schema>>>,
    pub not: Option<Box<Schema>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    pub schemas: HashMap<String, Schema>,
    pub responses: HashMap<String, Response>,
    pub parameters: HashMap<String, Parameter>,
    pub examples: HashMap<String, Example>,
    pub request_bodies: HashMap<String, RequestBody>,
    pub headers: HashMap<String, Header>,
    pub security_schemes: HashMap<String, SecurityScheme>,
    pub links: HashMap<String, Link>,
    pub callbacks: HashMap<String, Callback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub description: Option<String>,
    pub external_docs: Option<ExternalDocs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDocs {
    pub description: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub value: Option<serde_json::Value>,
    pub external_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub description: Option<String>,
    pub required: bool,
    pub deprecated: bool,
    pub allow_empty_value: bool,
    pub schema: Option<Schema>,
    pub example: Option<serde_json::Value>,
    pub examples: Option<HashMap<String, Example>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub operation_ref: Option<String>,
    pub operation_id: Option<String>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    pub request_body: Option<serde_json::Value>,
    pub description: Option<String>,
    pub server: Option<ApiServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Callback {
    pub expression: HashMap<String, PathItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRequirement {
    pub name: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScheme {
    #[serde(rename = "type")]
    pub scheme_type: SecuritySchemeType,
    pub description: Option<String>,
    pub name: Option<String>,
    pub in_location: Option<String>,
    pub scheme: Option<String>,
    pub bearer_format: Option<String>,
    pub flows: Option<OAuthFlows>,
    pub open_id_connect_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecuritySchemeType {
    ApiKey,
    Http,
    OAuth2,
    OpenIdConnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthFlows {
    pub implicit: Option<OAuthFlow>,
    pub password: Option<OAuthFlow>,
    pub client_credentials: Option<OAuthFlow>,
    pub authorization_code: Option<OAuthFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthFlow {
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub refresh_url: Option<String>,
    pub scopes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discriminator {
    pub property_name: String,
    pub mapping: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Xml {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub prefix: Option<String>,
    pub attribute: bool,
    pub wrapped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encoding {
    pub content_type: Option<String>,
    pub headers: Option<HashMap<String, Header>>,
    pub style: Option<String>,
    pub explode: bool,
    pub allow_reserved: bool,
}

// API documentation generator
pub struct ApiDocGenerator {
    endpoints: Vec<ApiEndpoint>,
    version: ApiVersion,
    config: DocGeneratorConfig,
}

#[derive(Debug, Clone)]
pub struct DocGeneratorConfig {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub contact_name: String,
    pub contact_email: String,
    pub license_name: String,
    pub license_url: String,
    pub include_examples: bool,
    pub include_schemas: bool,
}

impl Default for DocGeneratorConfig {
    fn default() -> Self {
        Self {
            title: "StratoSort API".to_string(),
            description: "File organization and management API".to_string(),
            base_url: "http://localhost:1420".to_string(),
            contact_name: "StratoSort Team".to_string(),
            contact_email: "support@stratosort.local".to_string(),
            license_name: "MIT".to_string(),
            license_url: "https://opensource.org/licenses/MIT".to_string(),
            include_examples: true,
            include_schemas: true,
        }
    }
}

impl ApiDocGenerator {
    pub fn new(endpoints: Vec<ApiEndpoint>, version: ApiVersion, config: DocGeneratorConfig) -> Self {
        Self {
            endpoints,
            version,
            config,
        }
    }

    // Generate OpenAPI specification
    pub fn generate_openapi_spec(&self) -> OpenApiSpec {
        OpenApiSpec {
            openapi: "3.0.0".to_string(),
            info: self.generate_info(),
            servers: self.generate_servers(),
            paths: self.generate_paths(),
            components: self.generate_components(),
            tags: self.generate_tags(),
            external_docs: Some(ExternalDocs {
                description: Some("Find more info here".to_string()),
                url: "https://github.com/stratosort/docs".to_string(),
            }),
        }
    }

    // Generate API info
    fn generate_info(&self) -> ApiInfo {
        ApiInfo {
            title: self.config.title.clone(),
            description: self.config.description.clone(),
            version: self.version.to_string(),
            terms_of_service: Some("https://stratosort.local/terms".to_string()),
            contact: Some(Contact {
                name: Some(self.config.contact_name.clone()),
                url: Some("https://stratosort.local".to_string()),
                email: Some(self.config.contact_email.clone()),
            }),
            license: Some(License {
                name: self.config.license_name.clone(),
                url: Some(self.config.license_url.clone()),
            }),
        }
    }

    // Generate server definitions
    fn generate_servers(&self) -> Vec<ApiServer> {
        vec![
            ApiServer {
                url: self.config.base_url.clone(),
                description: "Development server".to_string(),
                variables: HashMap::new(),
            },
            ApiServer {
                url: "https://api.stratosort.com".to_string(),
                description: "Production server".to_string(),
                variables: HashMap::new(),
            },
        ]
    }

    // Generate path definitions
    fn generate_paths(&self) -> HashMap<String, PathItem> {
        let mut paths = HashMap::new();

        for endpoint in &self.endpoints {
            let operation = self.generate_operation(endpoint);

            let path_item = paths.entry(endpoint.path.clone()).or_insert(PathItem {
                summary: Some(endpoint.description.clone()),
                description: Some(format!("Endpoint: {}", endpoint.path)),
                get: None,
                put: None,
                post: None,
                delete: None,
                options: None,
                head: None,
                patch: None,
                trace: None,
                servers: None,
                parameters: None,
            });

            // Set operation based on HTTP method
            match endpoint.method {
                HttpMethod::Get => path_item.get = Some(operation),
                HttpMethod::Post => path_item.post = Some(operation),
                HttpMethod::Put => path_item.put = Some(operation),
                HttpMethod::Delete => path_item.delete = Some(operation),
                HttpMethod::Patch => path_item.patch = Some(operation),
            }
        }

        paths
    }

    // Generate operation definition
    fn generate_operation(&self, endpoint: &ApiEndpoint) -> Operation {
        let mut responses = HashMap::new();

        // Success response
        responses.insert("200".to_string(), Response {
            description: "Successful response".to_string(),
            headers: None,
            content: if let Some(response_schema) = &endpoint.response_schema {
                let mut content = HashMap::new();
                content.insert("application/json".to_string(), MediaType {
                    schema: Some(self.json_value_to_schema(response_schema)),
                    example: self.config.include_examples.then(|| response_schema.clone()),
                    examples: None,
                    encoding: None,
                });
                Some(content)
            } else {
                None
            },
            links: None,
        });

        // Error responses
        responses.insert("400".to_string(), Response {
            description: "Bad Request".to_string(),
            headers: None,
            content: None,
            links: None,
        });

        responses.insert("500".to_string(), Response {
            description: "Internal Server Error".to_string(),
            headers: None,
            content: None,
            links: None,
        });

        Operation {
            tags: vec![self.extract_tag_from_path(&endpoint.path)],
            summary: endpoint.description.clone(),
            description: Some(format!("Handler: {}", endpoint.handler)),
            external_docs: None,
            operation_id: self.generate_operation_id(endpoint),
            parameters: self.extract_parameters(endpoint),
            request_body: endpoint.request_schema.as_ref().map(|schema| RequestBody {
                description: Some("Request body".to_string()),
                content: {
                    let mut content = HashMap::new();
                    content.insert("application/json".to_string(), MediaType {
                        schema: Some(self.json_value_to_schema(schema)),
                        example: self.config.include_examples.then(|| schema.clone()),
                        examples: None,
                        encoding: None,
                    });
                    content
                },
                required: true,
            }),
            responses,
            callbacks: None,
            deprecated: endpoint.deprecated_in.is_some(),
            security: None,
            servers: None,
        }
    }

    // Generate components section
    fn generate_components(&self) -> Components {
        Components {
            schemas: self.generate_schemas(),
            responses: HashMap::new(),
            parameters: HashMap::new(),
            examples: HashMap::new(),
            request_bodies: HashMap::new(),
            headers: HashMap::new(),
            security_schemes: self.generate_security_schemes(),
            links: HashMap::new(),
            callbacks: HashMap::new(),
        }
    }

    // Generate common schemas
    fn generate_schemas(&self) -> HashMap<String, Schema> {
        let mut schemas = HashMap::new();

        // Error schema
        schemas.insert("Error".to_string(), Schema {
            schema_type: Some("object".to_string()),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("code".to_string(), Box::new(Schema {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                }));
                props.insert("message".to_string(), Box::new(Schema {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                }));
                props
            }),
            required: Some(vec!["code".to_string(), "message".to_string()]),
            ..Default::default()
        });

        schemas
    }

    // Generate security schemes
    fn generate_security_schemes(&self) -> HashMap<String, SecurityScheme> {
        let mut schemes = HashMap::new();

        schemes.insert("csrfToken".to_string(), SecurityScheme {
            scheme_type: SecuritySchemeType::ApiKey,
            description: Some("CSRF token for state-modifying operations".to_string()),
            name: Some("X-CSRF-Token".to_string()),
            in_location: Some("header".to_string()),
            scheme: None,
            bearer_format: None,
            flows: None,
            open_id_connect_url: None,
        });

        schemes
    }

    // Generate tags
    fn generate_tags(&self) -> Vec<Tag> {
        let mut tag_set = std::collections::HashSet::new();

        for endpoint in &self.endpoints {
            tag_set.insert(self.extract_tag_from_path(&endpoint.path));
        }

        tag_set.into_iter().map(|name| Tag {
            name,
            description: None,
            external_docs: None,
        }).collect()
    }

    // Helper: Extract tag from path
    fn extract_tag_from_path(&self, path: &str) -> String {
        path.split('/').nth(1).unwrap_or("general").to_string()
    }

    // Helper: Generate operation ID
    fn generate_operation_id(&self, endpoint: &ApiEndpoint) -> String {
        format!("{}_{}",
            endpoint.method.as_str().to_lowercase(),
            endpoint.path.replace('/', "_").trim_matches('_')
        )
    }

    // Helper: Extract parameters from endpoint
    fn extract_parameters(&self, endpoint: &ApiEndpoint) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        // Extract path parameters
        for segment in endpoint.path.split('/') {
            if segment.starts_with(':') {
                let param_name = segment.trim_start_matches(':');
                parameters.push(Parameter {
                    name: param_name.to_string(),
                    in_location: ParameterLocation::Path,
                    description: Some(format!("Path parameter: {}", param_name)),
                    required: true,
                    deprecated: false,
                    allow_empty_value: false,
                    schema: Some(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    }),
                    example: None,
                    examples: None,
                });
            }
        }

        parameters
    }

    // Helper: Convert JSON value to OpenAPI schema
    #[allow(clippy::only_used_in_recursion)]
    fn json_value_to_schema(&self, value: &serde_json::Value) -> Schema {
        match value {
            serde_json::Value::Object(obj) => Schema {
                schema_type: Some("object".to_string()),
                properties: Some(obj.iter().map(|(k, v)| {
                    (k.clone(), Box::new(self.json_value_to_schema(v)))
                }).collect()),
                ..Default::default()
            },
            serde_json::Value::Array(arr) => Schema {
                schema_type: Some("array".to_string()),
                items: arr.first().map(|v| Box::new(self.json_value_to_schema(v))),
                ..Default::default()
            },
            serde_json::Value::String(_) => Schema {
                schema_type: Some("string".to_string()),
                ..Default::default()
            },
            serde_json::Value::Number(_) => Schema {
                schema_type: Some("number".to_string()),
                ..Default::default()
            },
            serde_json::Value::Bool(_) => Schema {
                schema_type: Some("boolean".to_string()),
                ..Default::default()
            },
            serde_json::Value::Null => Schema {
                nullable: true,
                ..Default::default()
            },
        }
    }

    // Generate HTML documentation
    pub fn generate_html_docs(&self) -> String {
        let spec = self.generate_openapi_spec();
        let spec_json = serde_json::to_string(&spec).unwrap_or_default();

        format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{} API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist/swagger-ui.css">
    <style>
        body {{
            margin: 0;
            padding: 0;
        }}
        #swagger-ui {{
            margin: 20px auto;
            max-width: 1200px;
        }}
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist/swagger-ui-bundle.js"></script>
    <script>
        const spec = {};
        window.onload = function() {{
            SwaggerUIBundle({{
                spec: spec,
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIBundle.SwaggerUIStandalonePreset
                ],
                layout: "BaseLayout"
            }});
        }};
    </script>
</body>
</html>"#, self.config.title, spec_json)
    }

    // Export specification to file
    pub async fn export_to_file(&self, path: &str, format: ExportFormat) -> Result<(), AppError> {
        let content = match format {
            ExportFormat::Json => {
                let spec = self.generate_openapi_spec();
                serde_json::to_string_pretty(&spec)
                    .map_err(|e| AppError::SerializationError {
                        message: format!("Failed to serialize OpenAPI spec: {}", e)
                    })?
            },
            ExportFormat::Yaml => {
                // Would require serde_yaml crate
                return Err(AppError::SystemError {
                    message: "YAML export not implemented yet".to_string()
                });
            },
            ExportFormat::Html => self.generate_html_docs(),
        };

        tokio::fs::write(path, content).await
            .map_err(|e| AppError::IoError {
                message: format!("Failed to write documentation file: {}", e)
            })?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Yaml,
    Html,
}


// Note: HttpMethod::as_str() is defined in versioning.rs