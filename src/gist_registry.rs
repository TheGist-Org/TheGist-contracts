use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, String, Vec};

/// Gist data structure representing a gist entry in the registry
#[derive(Clone)]
#[contracttype]
pub struct Gist {
    /// Auto-incrementing on-chain ID
    pub gist_id: u64,
    /// IPFS content identifier for the gist body
    pub ipfs_cid: Bytes,
    /// Geohash at precision 7 (~150m × 150m cell)
    pub geohash: String,
    /// Stellar address of the signing keypair
    pub author: Address,
    /// Ledger timestamp at submission
    pub timestamp: u64,
    /// Expiry timestamp (default: 24h from post)
    pub expiry: u64,
}

/// Event emitted when a gist is successfully posted
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum GistRegistryEvent {
    GistPosted(GistPostedEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct GistPostedEvent {
    pub gist_id: u64,
    pub author: Address,
    pub timestamp: u64,
}

/// GistRegistry contract - stores all gists posted to TheGist protocol
#[contract]
pub struct GistRegistry;

#[contractimpl]
impl GistRegistry {
    /// Post a new gist to the registry
    /// 
    /// # Arguments
    /// * `ipfs_cid` - IPFS content identifier for the gist body
    /// * `geohash` - Geohash at precision 7 (~150m × 150m cell)
    /// * `author` - Stellar address of the signing keypair
    /// * `expiry` - Expiry timestamp (optional, defaults to 24h from post)
    /// 
    /// # Returns
    /// The gist_id of the newly created gist
    pub fn post_gist(
        env: Env,
        ipfs_cid: Bytes,
        geohash: String,
        author: Address,
        expiry: Option<u64>,
    ) -> u64 {
        // Verify the author is the caller
        author.require_auth();

        // Get and increment the counter
        let counter_key = Bytes::from_slice(&env, b"counter");
        let gist_id: u64 = env.storage().get(&counter_key).unwrap_or(Ok(0u64)).unwrap();
        let new_gist_id = gist_id.checked_add(1).unwrap();
        env.storage().set(&counter_key, &new_gist_id);

        // Calculate expiry if not provided (default 24 hours)
        let timestamp = env.ledger().timestamp();
        let expiry = expiry.unwrap_or(timestamp + 86400); // 24 hours in seconds

        // Create the gist
        let gist = Gist {
            gist_id: new_gist_id,
            ipfs_cid,
            geohash,
            author: author.clone(),
            timestamp,
            expiry,
        };

        // Store the gist
        let gist_key = Bytes::from_slice(&env, &new_gist_id.to_be_bytes());
        env.storage().set(&gist_key, &gist);

        // Emit the event
        env.events().publish(
            GistRegistryEvent::GistPosted(GistPostedEvent {
                gist_id: new_gist_id,
                author,
                timestamp,
            }),
            (),
        );

        new_gist_id
    }

    /// Get a gist by its ID
    /// 
    /// # Arguments
    /// * `gist_id` - The ID of the gist to retrieve
    /// 
    /// # Returns
    /// The Gist struct if found, None otherwise
    pub fn get_gist(env: Env, gist_id: u64) -> Option<Gist> {
        let gist_key = Bytes::from_slice(&env, &gist_id.to_be_bytes());
        env.storage().get(&gist_key)
    }

    /// Get the total number of gists posted
    /// 
    /// # Returns
    /// The current counter value (total gists posted)
    pub fn get_gist_count(env: Env) -> u64 {
        let counter_key = Bytes::from_slice(&env, b"counter");
        env.storage().get(&counter_key).unwrap_or(Ok(0u64)).unwrap()
    }

    /// Get all gists by a specific author
    /// 
    /// # Arguments
    /// * `author` - The address of the author
    /// 
    /// # Returns
    /// A vector of gist IDs posted by the author
    pub fn get_gists_by_author(env: Env, author: Address) -> Vec<u64> {
        // Filter gists by author (this is a simplified implementation)
        // In production, you'd want a proper index
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

    /// Get all gists within a specific geohash region
    /// 
    /// # Arguments
    /// * `geohash_prefix` - The geohash prefix to filter by
    /// 
    /// # Returns
    /// A vector of gist IDs within the geohash region
    pub fn get_gists_by_geohash(env: Env, geohash_prefix: String) -> Vec<u64> {
        let count = Self::get_gist_count(env.clone());
        let mut result = Vec::new(&env);
        let prefix_bytes = geohash_prefix.to_bytes();
        let prefix_len = prefix_bytes.len();
        
        for id in 1..=count {
            if let Some(gist) = Self::get_gist(env.clone(), id) {
                // Check if gist geohash starts with the prefix by comparing bytes
                let gist_bytes = gist.geohash.to_bytes();
                if gist_bytes.len() >= prefix_len {
                    let matches = &gist_bytes[..prefix_len] == prefix_bytes.as_slice();
                    if matches {
                        result.push_back(id);
                    }
                }
            }
        }

        result
    }
}
