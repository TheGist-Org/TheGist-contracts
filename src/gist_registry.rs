use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String, Vec};

#[contracttype]
enum DataKey {
    Admin,
    GistCount,
    Gist(u64),
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

#[contract]
pub struct GistRegistry;

#[contractimpl]
impl GistRegistry {
    /// Set the admin address once on first deployment.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
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
            .instance()
            .get(&DataKey::Gist(gist_id))
            .expect("gist not found");
        if gist.author != caller {
            panic!("only the author can expire this gist");
        }
        gist.is_active = false;
        gist.expiry = env.ledger().timestamp();
        env.storage().instance().set(&DataKey::Gist(gist_id), &gist);
        env.events().publish(
            (symbol_short!("gist"), symbol_short!("expired")),
            GistExpiredEvent { gist_id, expired_by: caller },
        );
    }

    /// Admin expires any gist.
    pub fn admin_expire_gist(env: Env, admin: Address, gist_id: u64) {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");
        if stored != admin {
            panic!("caller is not the admin");
        }
        let mut gist: Gist = env
            .storage()
            .instance()
            .get(&DataKey::Gist(gist_id))
            .expect("gist not found");
        gist.is_active = false;
        env.storage().instance().set(&DataKey::Gist(gist_id), &gist);
        env.events().publish(
            (symbol_short!("gist"), symbol_short!("expired")),
            GistExpiredEvent { gist_id, expired_by: admin },
        );
    }

    /// Admin batch-expires up to 20 gists; returns count actually expired.
    pub fn batch_expire(env: Env, admin: Address, gist_ids: Vec<u64>) -> u32 {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");
        if stored != admin {
            panic!("caller is not the admin");
        }
        if gist_ids.len() > 20 {
            panic!("batch size exceeds maximum of 20");
        }
        let mut count: u32 = 0;
        for gist_id in gist_ids.iter() {
            if let Some(mut gist) = env
                .storage()
                .instance()
                .get::<DataKey, Gist>(&DataKey::Gist(gist_id))
            {
                gist.is_active = false;
                env.storage().instance().set(&DataKey::Gist(gist_id), &gist);
                env.events().publish(
                    (symbol_short!("gist"), symbol_short!("expired")),
                    GistExpiredEvent { gist_id, expired_by: admin.clone() },
                );
                count += 1;
            }
        }
        count
    }

    /// Returns false if the gist doesn't exist, was manually expired, or is past its TTL.
    pub fn is_gist_active(env: Env, gist_id: u64) -> bool {
        match env
            .storage()
            .instance()
            .get::<DataKey, Gist>(&DataKey::Gist(gist_id))
        {
            Some(gist) => gist.is_active && gist.expiry > env.ledger().timestamp(),
            None => false,
        }
    }

    pub fn post_gist(
        env: Env,
        ipfs_cid: Bytes,
        geohash: String,
        author: Address,
        expiry: Option<u64>,
    ) -> u64 {
        author.require_auth();

        let gist_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GistCount)
            .unwrap_or(0u64);
        let new_gist_id = gist_id.checked_add(1).unwrap();
        env.storage().instance().set(&DataKey::GistCount, &new_gist_id);

        let timestamp = env.ledger().timestamp();
        let expiry = expiry.unwrap_or(timestamp + 86400);

        let gist = Gist {
            gist_id: new_gist_id,
            ipfs_cid,
            geohash,
            author: author.clone(),
            timestamp,
            expiry,
            is_active: true,
        };

        env.storage().instance().set(&DataKey::Gist(new_gist_id), &gist);

        env.events().publish(
            (symbol_short!("gist"), symbol_short!("posted")),
            GistPostedEvent { gist_id: new_gist_id, author, timestamp },
        );

        new_gist_id
    }

    pub fn get_gist(env: Env, gist_id: u64) -> Option<Gist> {
        env.storage().instance().get(&DataKey::Gist(gist_id))
    }

    pub fn get_gist_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::GistCount).unwrap_or(0u64)
    }

    pub fn get_gists_by_author(env: Env, author: Address) -> Vec<u64> {
        let count = Self::get_gist_count(env.clone());
        let mut result = Vec::new(&env);
        for id in 1..=count {
            if let Some(gist) = Self::get_gist(env.clone(), id) {
                if gist.author == author {
                    result.push_back(id);
                }
            }
        }
        result
    }

    pub fn get_gists_by_geohash(env: Env, geohash_prefix: String) -> Vec<u64> {
        let count = Self::get_gist_count(env.clone());
        let mut result = Vec::new(&env);
        let prefix_len = geohash_prefix.len() as usize;
        let mut prefix_buf = [0u8; 12];
        geohash_prefix.copy_into_slice(&mut prefix_buf[..prefix_len]);

        for id in 1..=count {
            if let Some(gist) = Self::get_gist(env.clone(), id) {
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
