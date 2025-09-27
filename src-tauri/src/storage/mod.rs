mod database;
mod init;
mod vector_ext;

pub use database::{
    Database, Operation, CURRENT_SCHEMA_VERSION,
    is_valid_sql_identifier, validate_table_name, validate_column_name, escape_sql_string
};
pub use init::{
    check_vec_extension_availability, get_vector_config_for_model, initialize_sqlite_vec,
    VectorConfig,
};
pub use vector_ext::{ManualVectorSearch, VectorExtension, VectorStats};
