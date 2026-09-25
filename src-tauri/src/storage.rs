pub use lhc_core::storage::StoragePaths;

pub fn resolve_storage_paths() -> Result<StoragePaths, String> {
    StoragePaths::resolve()
}
