mod database;
mod vector_ext;
mod init;

pub use database::{Database, Operation, CURRENT_SCHEMA_VERSION};
pub use vector_ext::{VectorExtension, VectorStats, ManualVectorSearch};
pub use init::{initialize_sqlite_vec, check_vec_extension_availability, VectorConfig, get_vector_config_for_model};