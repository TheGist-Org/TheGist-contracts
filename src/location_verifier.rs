use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

#[contracttype]
enum DataKey {
    RegistryAddress,
    AllowedPrefixes,
}

/// LocationVerifier - Validates that a submitted geohash falls within an allowed geographic boundary
/// Used to enforce region-scoped deployments or to prevent spam from invalid coordinates
#[contract]
pub struct LocationVerifier;

impl LocationVerifier {
    fn read_allowed_prefixes(env: &Env) -> Vec<String> {
        env.storage()
            .instance()
            .get(&DataKey::AllowedPrefixes)
            .unwrap_or_else(|| Vec::new(env))
    }

    fn write_allowed_prefixes(env: &Env, prefixes: &Vec<String>) {
        env.storage().instance().set(&DataKey::AllowedPrefixes, prefixes);
    }

    fn matches_prefix(geohash: &String, prefix: &String) -> bool {
        let geohash_len = geohash.len() as usize;
        let prefix_len = prefix.len() as usize;
        if prefix_len > geohash_len {
            return false;
        }

        let mut geohash_buf = [0u8; 64];
        let mut prefix_buf = [0u8; 64];
        geohash.copy_into_slice(&mut geohash_buf[..geohash_len]);
        prefix.copy_into_slice(&mut prefix_buf[..prefix_len]);
        geohash_buf[..prefix_len] == prefix_buf[..prefix_len]
    }
}

#[contractimpl]
impl LocationVerifier {
    /// Initialize the verifier with boundary definitions.
    pub fn __init(env: Env) {
        if !env.storage().instance().has(&DataKey::AllowedPrefixes) {
            env.storage()
                .instance()
                .set(&DataKey::AllowedPrefixes, &Vec::<String>::new(&env));
        }
    }

    /// Store the GistRegistry contract address for deployment-time wiring.
    pub fn set_registry_address(env: Env, registry_address: Address) {
        env.storage().instance().set(&DataKey::RegistryAddress, &registry_address);
    }

    /// Read the configured GistRegistry contract address.
    pub fn get_registry_address(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryAddress)
    }

    /// Add a geohash prefix that should be considered valid.
    pub fn add_allowed_prefix(env: Env, prefix: String) {
        let mut prefixes = Self::read_allowed_prefixes(&env);
        prefixes.push_back(prefix);
        Self::write_allowed_prefixes(&env, &prefixes);
    }

    /// Verify if a geohash is within allowed boundaries.
    pub fn verify_geohash(env: Env, geohash: String) -> bool {
        let prefixes = Self::read_allowed_prefixes(&env);
        for prefix in prefixes.iter() {
            if Self::matches_prefix(&geohash, &prefix) {
                return true;
            }
        }
        false
    }

    /// Replace boundary definitions with a single prefix.
    pub fn update_boundaries(env: Env, boundaries: String) {
        let mut prefixes = Vec::new(&env);
        prefixes.push_back(boundaries);
        Self::write_allowed_prefixes(&env, &prefixes);
    }

    /// Get current boundary definitions.
    pub fn get_boundaries(env: Env) -> String {
        let prefixes = Self::read_allowed_prefixes(&env);
        if let Some(prefix) = prefixes.get(0) {
            prefix
        } else {
            String::from_slice(&env, "{}")
        }
    }
}
