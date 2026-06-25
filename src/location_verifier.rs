use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, Vec,
};

#[contracttype]
enum DataKey {
    RegistryAddress,
    AllowedPrefixes,
    Admin,
}

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
        env.storage()
            .instance()
            .set(&DataKey::AllowedPrefixes, prefixes);
    }

    fn require_admin(env: &Env, admin: &Address) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");

        if stored_admin != *admin {
            panic!("not admin");
        }
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

    fn is_valid_geohash_format(location_cell: &String) -> bool {
        if location_cell.len() != 7 {
            return false;
        }

        let valid_chars = b"0123456789bcdefghjkmnpqrstuvwxyz";

        let len = location_cell.len() as usize;
        let mut buffer = [0u8; 64];

        location_cell.copy_into_slice(&mut buffer[..len]);

        for i in 0..len {
            if !valid_chars.contains(&buffer[i]) {
                return false;
            }
        }

        true
    }
}

#[contractimpl]
impl LocationVerifier {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);

        if !env.storage().instance().has(&DataKey::AllowedPrefixes) {
            env.storage()
                .instance()
                .set(&DataKey::AllowedPrefixes, &Vec::<String>::new(&env));
        }
    }

    pub fn set_registry_address(
        env: Env,
        registry_address: Address,
    ) {
        env.storage()
            .instance()
            .set(&DataKey::RegistryAddress, &registry_address);
    }

    pub fn get_registry_address(env: Env) -> Option<Address> {
        env.storage()
            .instance()
            .get(&DataKey::RegistryAddress)
    }

    pub fn add_allowed_prefix(
        env: Env,
        admin: Address,
        prefix: String,
    ) {
        Self::require_admin(&env, &admin);

        let len = prefix.len();

        if len < 1 || len > 6 {
            panic!("invalid prefix length");
        }

        let mut prefixes = Self::read_allowed_prefixes(&env);

        for existing in prefixes.iter() {
            if existing == prefix {
                return;
            }
        }

        prefixes.push_back(prefix);

        Self::write_allowed_prefixes(&env, &prefixes);
    }

    pub fn remove_allowed_prefix(
        env: Env,
        admin: Address,
        prefix: String,
    ) {
        Self::require_admin(&env, &admin);

        let current = Self::read_allowed_prefixes(&env);

        let mut updated = Vec::new(&env);

        for item in current.iter() {
            if item != prefix {
                updated.push_back(item);
            }
        }

        Self::write_allowed_prefixes(&env, &updated);
    }

    pub fn get_allowed_prefixes(env: Env) -> Vec<String> {
        Self::read_allowed_prefixes(&env)
    }

    pub fn is_valid_geohash(
        env: Env,
        location_cell: String,
    ) -> bool {
        if !Self::is_valid_geohash_format(&location_cell) {
            return false;
        }

        let prefixes = Self::read_allowed_prefixes(&env);

        for prefix in prefixes.iter() {
            if Self::matches_prefix(&location_cell, &prefix) {
                return true;
            }
        }

        false
    }

    pub fn update_boundaries(
        env: Env,
        boundaries: String,
    ) {
        let mut prefixes = Vec::new(&env);

        prefixes.push_back(boundaries);

        Self::write_allowed_prefixes(&env, &prefixes);
    }

    pub fn get_boundaries(env: Env) -> String {
        let prefixes = Self::read_allowed_prefixes(&env);

        if let Some(prefix) = prefixes.get(0) {
            prefix
        } else {
            String::from_str(&env, "{}")
        }
    }
}