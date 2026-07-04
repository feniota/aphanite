#[derive(Default)]
pub struct AssetsStorage;

impl AssetsStorage {
    pub fn whitelist_domain(&self) -> Option<String> {
        None
    }
}
