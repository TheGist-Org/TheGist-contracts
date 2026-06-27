use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String, Vec};

const CONTRACT_VERSION: u32 = 2;
const LEDGERS_PER_HOUR: u32 = 720;
const SECONDS_PER_HOUR: u64 = 3600;
const DEFAULT_GIST_TTL_HOURS: u32 = 24;
const MAX_GIST_TTL_HOURS: u32 = 24 * 7;
const AUTHOR_LIST_TTL_HOURS: u32 = 24 * 30;

#[contracttype]
enum DataKey {
    Admin,
    GistCount,
    ContractVersion,
    Gist(u64),
    AuthorGists(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct Gist {
    pub gist_id: u64,
    pub ipfs_cid: Bytes,
    pub geohash: String,
    pub author: Address,
    pub timestamp: u64,
    pub expiry: u64,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct GistPostedEvent {
    pub gist_id: u64,
    pub author: Address,
    pub timestamp: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct GistExpiredEvent {
    pub gist_id: u64,
    pub expired_by: Address,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ContractUpgradedEvent {
    pub old_version: u32,
    pub new_version: u32,
}

#[contract]
pub struct GistRegistry;

impl GistRegistry {
    fn gist_key(gist_id: u64) -> DataKey {
        DataKey::Gist(gist_id)
    }

    fn author_gists_key(author: &Address) -> DataKey {
        DataKey::AuthorGists(author.clone())
    }

    fn hours_to_ledger_ttl(ttl_hours: u32) -> u32 {
        ttl_hours
            .checked_mul(LEDGERS_PER_HOUR)
            .expect("ttl too large")
    }

    fn seconds_to_ledger_ttl(ttl_seconds: u64) -> u32 {
        let ttl_hours = ttl_seconds
            .checked_add(SECONDS_PER_HOUR - 1)
            .expect("ttl too large")
            / SECONDS_PER_HOUR;
        Self::hours_to_ledger_ttl(ttl_hours as u32)
    }

    fn gist_time_to_expiry(env: &Env, ttl_or_expiry: Option<u64>) -> (u64, u32) {
        let now = env.ledger().timestamp();
        let ttl_hours = match ttl_or_expiry {
            Some(value) if value <= MAX_GIST_TTL_HOURS as u64 => value as u32,
            Some(expiry) => {
                let ttl_seconds = expiry
                    .checked_sub(now)
                    .expect("expiry must be in the future");
                return (expiry, Self::seconds_to_ledger_ttl(ttl_seconds));
            }
            None => DEFAULT_GIST_TTL_HOURS,
        };

        let ttl_seconds = u64::from(ttl_hours)
            .checked_mul(SECONDS_PER_HOUR)
            .expect("ttl too large");
        let expiry = now
            .checked_add(ttl_seconds)
            .expect("expiry too large");
        (expiry, Self::hours_to_ledger_ttl(ttl_hours))
    }

    fn load_gist(env: &Env, gist_id: u64) -> Option<Gist> {
        env.storage()
            .temporary()
            .get(&Self::gist_key(gist_id))
    }

    fn store_gist(env: &Env, gist: &Gist, ledger_ttl: u32) {
        let key = Self::gist_key(gist.gist_id);
        env.storage().temporary().set(&key, gist);
        env.storage().temporary().extend_ttl(&key, ledger_ttl, ledger_ttl);
    }

    fn append_author_gist(env: &Env, author: &Address, gist_id: u64) {
        let key = Self::author_gists_key(author);
        let mut gist_ids = env
            .storage()
            .temporary()
            .get::<DataKey, Vec<u64>>(&key)
            .unwrap_or_else(|| Vec::new(env));
        gist_ids.push_back(gist_id);
        env.storage().temporary().set(&key, &gist_ids);
        let ttl = Self::hours_to_ledger_ttl(AUTHOR_LIST_TTL_HOURS);
        env.storage().temporary().extend_ttl(&key, ttl, ttl);
    }

    fn update_gist(env: &Env, gist: &Gist) {
        let key = Self::gist_key(gist.gist_id);
        env.storage().temporary().set(&key, gist);
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

    fn read_gist_count(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::GistCount)
            .unwrap_or(0u64)
    }
}

#[contractimpl]
impl GistRegistry {
    /// Set the admin address once on first deployment.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ContractVersion, &CONTRACT_VERSION);
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    pub fn get_version(_: Env) -> u32 {
        1
    }

    pub fn get_contract_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContractVersion)
            .unwrap_or(CONTRACT_VERSION)
    }

    /// Transfer admin role; requires current admin's auth.
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");
        if stored != current_admin {
            panic!("caller is not the current admin");
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// Author expires their own gist early.
    pub fn expire_gist(env: Env, caller: Address, gist_id: u64) {
        caller.require_auth();
        let mut gist: Gist = env
            .storage()
            .temporary()
            .get(&Self::gist_key(gist_id))
            .expect("gist not found");
        if gist.author != caller {
            panic!("only the author can expire this gist");
        }
        gist.is_active = false;
        gist.expiry = env.ledger().timestamp();
        Self::update_gist(&env, &gist);
        env.events().publish(
            (symbol_short!("gist"), symbol_short!("expired")),
            GistExpiredEvent { gist_id, expired_by: caller },
        );
    }

    /// Admin expires any gist.
    pub fn admin_expire_gist(env: Env, admin: Address, gist_id: u64) {
        Self::ensure_admin(&env, &admin);
        let mut gist: Gist = env
            .storage()
            .temporary()
            .get(&Self::gist_key(gist_id))
            .expect("gist not found");
        gist.is_active = false;
        gist.expiry = env.ledger().timestamp();
        Self::update_gist(&env, &gist);
        env.events().publish(
            (symbol_short!("gist"), symbol_short!("expired")),
            GistExpiredEvent { gist_id, expired_by: admin },
        );
    }

    /// Admin batch-expires up to 20 gists; returns count actually expired.
    pub fn batch_expire(env: Env, admin: Address, gist_ids: Vec<u64>) -> u32 {
        Self::ensure_admin(&env, &admin);
        if gist_ids.len() > 20 {
            panic!("batch size exceeds maximum of 20");
        }
        let mut count: u32 = 0;
        for gist_id in gist_ids.iter() {
            if let Some(mut gist) = Self::load_gist(&env, gist_id) {
                gist.is_active = false;
                gist.expiry = env.ledger().timestamp();
                Self::update_gist(&env, &gist);
                env.events().publish(
                    (symbol_short!("gist"), symbol_short!("expired")),
                    GistExpiredEvent { gist_id, expired_by: admin.clone() },
                );
                count += 1;
            }
        }
        count
    }

    /// Extends a gist by another 24 hours, capped at 7 days total.
    pub fn extend_gist_ttl(env: Env, gist_id: u64) {
        let mut gist: Gist = env
            .storage()
            .temporary()
            .get(&Self::gist_key(gist_id))
            .expect("gist not found");
        gist.author.require_auth();

        if !gist.is_active {
            panic!("cannot extend an inactive gist");
        }

        let max_expiry = gist
            .timestamp
            .checked_add(u64::from(MAX_GIST_TTL_HOURS) * SECONDS_PER_HOUR)
            .expect("expiry too large");
        let new_expiry = gist
            .expiry
            .checked_add(24 * SECONDS_PER_HOUR)
            .expect("expiry too large");
        if new_expiry > max_expiry {
            panic!("gist ttl exceeds maximum of 7 days");
        }

        gist.expiry = new_expiry;
        Self::update_gist(&env, &gist);

        let remaining_seconds = new_expiry
            .checked_sub(env.ledger().timestamp())
            .expect("gist already expired");
        let ledger_ttl = Self::seconds_to_ledger_ttl(remaining_seconds);
        let key = Self::gist_key(gist_id);
        env.storage().temporary().extend_ttl(&key, ledger_ttl, ledger_ttl);
    }

    /// Returns false if the gist doesn't exist, was manually expired, or is past its TTL.
    pub fn is_gist_active(env: Env, gist_id: u64) -> bool {
        match Self::load_gist(&env, gist_id) {
            Some(gist) => gist.is_active && gist.expiry > env.ledger().timestamp(),
            None => false,
        }
    }

    pub fn post_gist(
        env: Env,
        ipfs_cid: Bytes,
        geohash: String,
        author: Address,
        ttl_or_expiry: Option<u64>,
    ) -> u64 {
        author.require_auth();

        let gist_id = Self::read_gist_count(&env);
        let new_gist_id = gist_id.checked_add(1).unwrap();
        env.storage().instance().set(&DataKey::GistCount, &new_gist_id);

        let timestamp = env.ledger().timestamp();
        if let Some(expiry_input) = ttl_or_expiry {
    if expiry_input <= timestamp {
        panic!("expiry must be in the future");
    }

    if expiry_input > timestamp + 604_800 {
        panic!("expiry cannot exceed 168 hours from now");
    }
}
        let (expiry, ledger_ttl) = Self::gist_time_to_expiry(&env, ttl_or_expiry);
        if ipfs_cid.len() == 0 {
            panic!("ipfs_cid cannot be empty");
        }
        if geohash.len() != 7 {
            panic!("geohash must be exactly 7 characters");
        }

        let (expiry, ledger_ttl) = Self::gist_time_to_expiry(&env, ttl_or_expiry);
        let timestamp = env.ledger().timestamp();
        if expiry <= timestamp {
            panic!("expiry must be in the future");
        }
        if expiry > timestamp + 604800 {
            panic!("expiry cannot exceed 168 hours from now");
        }

        let gist = Gist {
            gist_id: new_gist_id,
            ipfs_cid,
            geohash,
            author: author.clone(),
            timestamp,
            expiry,
            is_active: true,
        };

        Self::store_gist(&env, &gist, ledger_ttl);
        Self::append_author_gist(&env, &author, new_gist_id);

        env.events().publish(
            (symbol_short!("gist"), symbol_short!("posted")),
            GistPostedEvent { gist_id: new_gist_id, author, timestamp },
        );

        new_gist_id
    }

    pub fn get_gist(env: Env, gist_id: u64) -> Option<Gist> {
        Self::load_gist(&env, gist_id)
    }

    pub fn get_gist_count(env: Env) -> u64 {
        Self::read_gist_count(&env)
    }

    pub fn get_gists_by_author(env: Env, author: Address, limit: u32, offset: u32) -> Vec<u64> {
        if limit > 50 {
            panic!("limit exceeds maximum of 50");
        }
        let mut result = Vec::new(&env);
        let mut skipped: u32 = 0;
        if let Some(gist_ids) = env
            .storage()
            .temporary()
            .get::<DataKey, Vec<u64>>(&Self::author_gists_key(&author))
        {
            for gist_id in gist_ids.iter() {
                if result.len() as u32 >= limit {
                    break;
                }
                if Self::load_gist(&env, gist_id).is_some() {
                    if skipped < offset {
                        skipped += 1;
                    } else {
                        result.push_back(gist_id);
                    }
                }
            }
        }
        result
    }

    pub fn get_active_gist_count(env: Env) -> u64 {
        let count = Self::read_gist_count(&env);
        let now = env.ledger().timestamp();
        let mut active: u64 = 0;
        for id in 1..=count {
            if let Some(gist) = Self::load_gist(&env, id) {
                if gist.is_active && gist.expiry > now {
                    active += 1;
                }
            }
        }
        active
    }

    pub fn get_gists_by_geohash(env: Env, geohash_prefix: String) -> Vec<u64> {
        let count = Self::read_gist_count(&env);
        let mut result = Vec::new(&env);
        let prefix_len = geohash_prefix.len() as usize;
        let mut prefix_buf = [0u8; 12];
        geohash_prefix.copy_into_slice(&mut prefix_buf[..prefix_len]);

        for id in 1..=count {
            if let Some(gist) = Self::load_gist(&env, id) {
                let gist_len = gist.geohash.len() as usize;
                if gist_len >= prefix_len {
                    let mut gist_buf = [0u8; 12];
                    gist.geohash.copy_into_slice(&mut gist_buf[..gist_len]);
                    if gist_buf[..prefix_len] == prefix_buf[..prefix_len] {
                        result.push_back(id);
                    }
                }
            }
        }
        result
    }
}
