// API Module
// Central module for API management and versioning

pub mod documentation;
pub mod versioning;

pub use documentation::{
    ApiDocGenerator,
    DocGeneratorConfig,
    ExportFormat,
    OpenApiSpec,
};

pub use versioning::{
    ApiVersion,
    ApiVersionManager,
    ApiEndpoint,
    ApiDocumentation,
    CompatibilityMatrix,
    DeprecationPolicy,
    HttpMethod,
    VersionMigration,
};