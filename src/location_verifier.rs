use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, String, Vec};

#[contracttype]
enum DataKey {
    Admin,
    RegistryAddress,
    AllowedPrefixes,
}

/// LocationVerifier - Validates that a submitted geohash falls within an allowed geographic boundary.
/// Used to enforce region-scoped deployments or to prevent spam from invalid coordinates.
#[contract]
pub struct LocationVerifier;

// Valid geohash base-32 alphabet (excludes a, i, l, o)
const GEOHASH_ALPHABET: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

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

    fn ensure_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("contract not initialized");
        if stored != *caller {
            panic!("caller is not the admin");
        }
    }

    fn valid_geohash_chars(s: &[u8]) -> bool {
        for &b in s {
            if !GEOHASH_ALPHABET.contains(&b) {
                return false;
            }
        }
        true
    }
}

#[contractimpl]
impl LocationVerifier {
    /// Initialize the verifier with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AllowedPrefixes, &Vec::<String>::new(&env));
    }

    /// Get the admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Store the GistRegistry contract address for cross-contract calls.
    pub fn set_registry_address(env: Env, admin: Address, registry_address: Address) {
        Self::ensure_admin(&env, &admin);
        env.storage().instance().set(&DataKey::RegistryAddress, &registry_address);
    }

    /// Read the configured GistRegistry contract address.
    pub fn get_registry_address(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryAddress)
    }

    /// Validate that a geohash is exactly 7 chars of valid geohash base-32 alphabet.
    pub fn is_valid_geohash(env: Env, geohash: String) -> bool {
        if geohash.len() != 7 {
            return false;
        }
        let mut buf = [0u8; 7];
        geohash.copy_into_slice(&mut buf);
        Self::valid_geohash_chars(&buf)
    }

    /// Admin adds a geohash prefix (1–6 chars) to the allowed list.
    pub fn add_allowed_prefix(env: Env, admin: Address, prefix: String) {
        Self::ensure_admin(&env, &admin);
        let len = prefix.len() as usize;
        if len == 0 {
            panic!("prefix cannot be empty");
        }
        if len > 6 {
            panic!("prefix length must not exceed 6");
        }
        let mut prefixes = Self::read_allowed_prefixes(&env);
        prefixes.push_back(prefix);
        Self::write_allowed_prefixes(&env, &prefixes);
    }

    /// Admin removes an existing prefix from the allowed list.
    pub fn remove_allowed_prefix(env: Env, admin: Address, prefix: String) {
        Self::ensure_admin(&env, &admin);
        let prefixes = Self::read_allowed_prefixes(&env);
        let mut new_prefixes = Vec::new(&env);
        let mut found = false;

        let target_len = prefix.len() as usize;
        let mut target_buf = [0u8; 64];
        prefix.copy_into_slice(&mut target_buf[..target_len]);

        for p in prefixes.iter() {
            let p_len = p.len() as usize;
            let mut p_buf = [0u8; 64];
            p.copy_into_slice(&mut p_buf[..p_len]);
            if !found && p_len == target_len && p_buf[..p_len] == target_buf[..target_len] {
                found = true; // skip first match (remove it)
            } else {
                new_prefixes.push_back(p);
            }
        }

        if !found {
            panic!("prefix not found");
        }
        Self::write_allowed_prefixes(&env, &new_prefixes);
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

    /// Verify geohash validity and allowed boundary, then post to GistRegistry.
    /// Returns the new gist_id.
    pub fn verify_and_post(
        env: Env,
        ipfs_cid: Bytes,
        geohash: String,
        author: Address,
        ttl_or_expiry: Option<u64>,
    ) -> u64 {
        author.require_auth();
        // Validate geohash format
        if !Self::is_valid_geohash(env.clone(), geohash.clone()) {
            panic!("invalid geohash");
        }
        // Validate geohash is within allowed boundaries
        if !Self::verify_geohash(env.clone(), geohash.clone()) {
            panic!("geohash not in allowed boundaries");
        }
        // Cross-contract call to GistRegistry
        let registry_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryAddress)
            .expect("registry address not set");
        let client = crate::gist_registry::GistRegistryClient::new(&env, &registry_address);
        client.post_gist(&ipfs_cid, &geohash, &author, &ttl_or_expiry)
    }
}
