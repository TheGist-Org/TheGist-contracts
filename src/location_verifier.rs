use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, String, Vec};

#[contracttype]
enum DataKey {
    Admin,
    RegistryAddress,
    AllowedPrefixes,
}

/// LocationVerifier - Validates that a submitted geohash falls within an allowed geographic boundary.
#[contract]
pub struct LocationVerifier;

// Valid base-32 geohash alphabet (excludes a, i, l, o)
const GEOHASH_CHARS: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

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

    fn ensure_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set");
        if admin != *caller {
            panic!("caller is not the admin");
        }
    }

    fn matches_prefix(geohash: &String, prefix: &String) -> bool {
        let gh_len = geohash.len() as usize;
        let p_len = prefix.len() as usize;
        if p_len > gh_len {
            return false;
        }
        let mut gh_buf = [0u8; 64];
        let mut p_buf = [0u8; 64];
        geohash.copy_into_slice(&mut gh_buf[..gh_len]);
        prefix.copy_into_slice(&mut p_buf[..p_len]);
        gh_buf[..p_len] == p_buf[..p_len]
    }

    fn ensure_admin(env: &Env, admin: &Address) {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");
        if stored != *admin {
            panic!("caller is not the admin");
        }
    }
}

#[contractimpl]
impl LocationVerifier {
    /// Initialize with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AllowedPrefixes, &Vec::<String>::new(&env));
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Store the GistRegistry contract address.
    pub fn set_registry_address(env: Env, registry_address: Address) {
        env.storage().instance().set(&DataKey::RegistryAddress, &registry_address);
    }

    /// Returns the configured GistRegistry contract address.
    pub fn get_registry_address(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryAddress)
    }

    /// Validate a geohash: exactly 7 chars, only valid base-32 alphabet.
    pub fn is_valid_geohash(env: Env, geohash: String) -> bool {
        if geohash.len() != 7 {
            return false;
        }
        let mut buf = [0u8; 7];
        geohash.copy_into_slice(&mut buf);
        for &b in buf.iter() {
            if !GEOHASH_CHARS.contains(&b) {
                return false;
            }
        }
        // Must also fall within an allowed prefix
        let prefixes = Self::read_allowed_prefixes(&env);
        for prefix in prefixes.iter() {
            if Self::matches_prefix(&geohash, &prefix) {
                return true;
            }
        }
        false
    }

    /// Admin: add a geohash prefix (max 6 chars, non-empty).
    pub fn add_allowed_prefix(env: Env, caller: Address, prefix: String) {
        Self::ensure_admin(&env, &caller);
        let len = prefix.len() as usize;
        if len == 0 {
            panic!("prefix cannot be empty");
        }
        if len > 6 {
            panic!("prefix length cannot exceed 6");
        }
        let mut prefixes = Self::read_allowed_prefixes(&env);
        prefixes.push_back(prefix.clone());
        Self::write_allowed_prefixes(&env, &prefixes);

        env.events().publish(
            (symbol_short!("location"), symbol_short!("pfx_add")),
            PrefixAddedEvent { prefix },
        );
    }

    /// Admin: remove an existing prefix. Panics if not found.
    pub fn remove_allowed_prefix(env: Env, caller: Address, prefix: String) {
        Self::ensure_admin(&env, &caller);
        let prefixes = Self::read_allowed_prefixes(&env);
        let mut new_prefixes = Vec::new(&env);
        let mut found = false;
        for p in prefixes.iter() {
            if p == prefix {
                found = true;
            } else {
                new_prefixes.push_back(p);
            }
        }
        if !found {
            panic!("prefix not found");
        }
        Self::write_allowed_prefixes(&env, &new_prefixes);
    }

    /// Verify geohash and, if valid, call GistRegistry.post_gist and return the gist_id.
    pub fn verify_and_post(
        env: Env,
        geohash: String,
        ipfs_cid: Bytes,
        author: Address,
        ttl_or_expiry: Option<u64>,
    ) -> u64 {
        if !Self::is_valid_geohash(env.clone(), geohash.clone()) {
            panic!("geohash is not valid or not in an allowed region");
        }
        let registry_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryAddress)
            .expect("registry address not set");

        let client = crate::GistRegistryClient::new(&env, &registry_address);
        client.post_gist(&ipfs_cid, &geohash, &author, &ttl_or_expiry)
    }

    // Legacy helpers kept for backward-compat ----------------------------------------

    /// Verify if a geohash is within allowed boundaries (no character validation).
    pub fn verify_geohash(env: Env, geohash: String) -> bool {
        let prefixes = Self::read_allowed_prefixes(&env);
        for prefix in prefixes.iter() {
            if Self::matches_prefix(&geohash, &prefix) {
                return true;
            }
        }
        false
    }

    /// Replace all boundary definitions with a single prefix.
    pub fn update_boundaries(env: Env, boundaries: String) {
        let mut prefixes = Vec::new(&env);
        prefixes.push_back(boundary);
        Self::write_allowed_prefixes(&env, &prefixes);
    }

    /// Get first boundary definition.
    pub fn get_boundaries(env: Env) -> String {
        let prefixes = Self::read_allowed_prefixes(&env);
        if let Some(prefix) = prefixes.get(0) {
            prefix
        } else {
            String::from_slice(&env, "")
        }
    }

    /// Validates geohash then cross-contract calls GistRegistry.post_gist atomically.
    pub fn verify_and_post(
        env: Env,
        author: Address,
        ipfs_cid: Bytes,
        location_cell: String,
        ttl_or_expiry: Option<u64>,
    ) -> u64 {
        author.require_auth();

        if !Self::is_valid_geohash(env.clone(), location_cell.clone()) {
            panic!("invalid or disallowed location cell");
        }

        let registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryAddress)
            .expect("registry address not set");

        env.invoke_contract(
            &registry,
            &soroban_sdk::symbol_short!("post_gist"),
            soroban_sdk::vec![
                &env,
                ipfs_cid.into_val(&env),
                location_cell.into_val(&env),
                author.into_val(&env),
                ttl_or_expiry.into_val(&env),
            ],
        )
    }
}
