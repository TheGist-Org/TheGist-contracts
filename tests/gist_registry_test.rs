use soroban_sdk::{Address, Bytes, Env, String};
use the_gist_contracts::{Gist, GistRegistry};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Verify counter starts at 0 (no initialization needed)
    assert_eq!(client.get_gist_count(), 0);
}

#[test]
fn test_post_gist() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, b"u4pruyd");

    // Post a gist
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, None);

    // Verify gist was created
    assert_eq!(gist_id, 1);
    assert_eq!(client.get_gist_count(), 1);

    // Verify gist can be retrieved
    let gist = client.get_gist(gist_id).unwrap();
    assert_eq!(gist.gist_id, 1);
    assert_eq!(gist.ipfs_cid, ipfs_cid);
    assert_eq!(gist.geohash, geohash);
    assert_eq!(gist.author, author);
}

#[test]
fn test_post_multiple_gists() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid1 = Bytes::from_slice(&env, b"QmTest123");
    let ipfs_cid2 = Bytes::from_slice(&env, b"QmTest456");
    let geohash1 = String::from_slice(&env, b"u4pruyd");
    let geohash2 = String::from_slice(&env, b"u4pruyd");

    // Post two gists
    let gist_id1 = client.post_gist(&ipfs_cid1, &geohash1, &author1, None);
    let gist_id2 = client.post_gist(&ipfs_cid2, &geohash2, &author2, None);

    // Verify gist IDs are sequential
    assert_eq!(gist_id1, 1);
    assert_eq!(gist_id2, 2);
    assert_eq!(client.get_gist_count(), 2);
}

#[test]
fn test_get_gist_not_found() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Initialize the contract
    client.__init();

    // Try to get a non-existent gist
    let gist = client.get_gist(999);
    assert!(gist.is_none());
}

#[test]
fn test_get_gists_by_author() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, b"u4pruyd");

    // Post gists from different authors
    client.post_gist(&ipfs_cid, &geohash, &author1, None);
    client.post_gist(&ipfs_cid, &geohash, &author2, None);
    client.post_gist(&ipfs_cid, &geohash, &author1, None);

    // Get gists by author1
    let author1_gists = client.get_gists_by_author(&author1);
    assert_eq!(author1_gists.len(), 2);
}

#[test]
fn test_get_gists_by_geohash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash1 = String::from_slice(&env, b"u4pruyd");
    let geohash2 = String::from_slice(&env, b"u4pruyd");

    // Post gists with different geohashes
    client.post_gist(&ipfs_cid, &geohash1, &author, None);
    client.post_gist(&ipfs_cid, &geohash2, &author, None);

    // Get gists by geohash prefix
    let gists = client.get_gists_by_geohash(&String::from_slice(&env, b"u4pruy"));
    assert_eq!(gists.len(), 2);
}

#[test]
fn test_custom_expiry() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, b"u4pruyd");
    let custom_expiry = env.ledger().timestamp() + 172800; // 48 hours

    // Post a gist with custom expiry
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, Some(custom_expiry));

    // Verify gist was created with custom expiry
    let gist = client.get_gist(gist_id).unwrap();
    assert_eq!(gist.expiry, custom_expiry);
}
