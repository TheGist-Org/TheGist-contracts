use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String, Vec};

#[contracttype]
enum DataKey {
    Admin,
    RegistryAddress,
    AllowedPrefixes,
}

/// LocationVerifier — validates geohash boundaries and optionally proxies post_gist.
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
    /// Initialize with the admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AllowedPrefixes, &Vec::<String>::new(&env));
    }

    /// Admin sets the GistRegistry contract address for cross-contract calls.
    pub fn set_registry_address(env: Env, admin: Address, registry: Address) {
        Self::ensure_admin(&env, &admin);
        env.storage().instance().set(&DataKey::RegistryAddress, &registry);

        env.events().publish(
            (symbol_short!("location"), symbol_short!("reg_set")),
            registry,
        );
    }

    /// Returns the configured GistRegistry contract address.
    pub fn get_registry_address(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryAddress)
    }

    /// Add a geohash prefix to the allowed list.
    pub fn add_allowed_prefix(env: Env, prefix: String) {
        let mut prefixes = Self::read_allowed_prefixes(&env);
        prefixes.push_back(prefix.clone());
        Self::write_allowed_prefixes(&env, &prefixes);

        env.events().publish(
            (symbol_short!("location"), symbol_short!("pfx_add")),
            prefix,
        );
    }

    /// Remove a geohash prefix from the allowed list.
    pub fn remove_allowed_prefix(env: Env, prefix: String) {
        let prefixes = Self::read_allowed_prefixes(&env);
        let mut new_prefixes = Vec::new(&env);
        for p in prefixes.iter() {
            if p != prefix {
                new_prefixes.push_back(p);
            }
        }
        Self::write_allowed_prefixes(&env, &new_prefixes);

        env.events().publish(
            (symbol_short!("location"), symbol_short!("pfx_rm")),
            prefix,
        );
    }

    /// Returns true if geohash matches any allowed prefix.
    pub fn is_valid_geohash(env: Env, geohash: String) -> bool {
        let prefixes = Self::read_allowed_prefixes(&env);
        for prefix in prefixes.iter() {
            if Self::matches_prefix(&geohash, &prefix) {
                return true;
            }
        }
        false
    }

    /// Legacy alias kept for compatibility.
    pub fn verify_geohash(env: Env, geohash: String) -> bool {
        Self::is_valid_geohash(env, geohash)
    }

    /// Replace all allowed prefixes with a single new value.
    pub fn update_boundaries(env: Env, boundary: String) {
        let mut prefixes = Vec::new(&env);
        prefixes.push_back(boundary);
        Self::write_allowed_prefixes(&env, &prefixes);
    }

    /// Returns the first allowed prefix, or an empty string if none set.
    pub fn get_boundaries(env: Env) -> String {
        let prefixes = Self::read_allowed_prefixes(&env);
        if let Some(prefix) = prefixes.get(0) {
            prefix
        } else {
            String::from_slice(&env, "")
        }
    }

    /// Validates geohash, then cross-contract calls GistRegistry.post_gist atomically.
    /// Returns the gist_id assigned by GistRegistry. Reverts entirely if geohash is invalid.
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

        let gist_id: u64 = env.invoke_contract(
            &registry,
            &soroban_sdk::symbol_short!("post_gist"),
            soroban_sdk::vec![
                &env,
                ipfs_cid.into_val(&env),
                location_cell.into_val(&env),
                author.into_val(&env),
                ttl_or_expiry.into_val(&env),
            ],
        );

        gist_id
    }
}
