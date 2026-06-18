use soroban_sdk::{vec, Address, Bytes, Env, String};
use soroban_sdk::testutils::Address as _;
use the_gist_contracts::GistRegistry;

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
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    // Post a gist
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Verify gist was created
    assert_eq!(gist_id, 1);
    assert_eq!(client.get_gist_count(), 1);

    // Verify gist can be retrieved
    let gist = client.get_gist(&gist_id).unwrap();
    assert_eq!(gist.gist_id, 1);
    assert_eq!(gist.ipfs_cid, ipfs_cid);
    assert_eq!(gist.geohash, geohash);
    assert_eq!(gist.author, author);
}

#[test]
fn test_post_multiple_gists() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid1 = Bytes::from_slice(&env, b"QmTest123");
    let ipfs_cid2 = Bytes::from_slice(&env, b"QmTest456");
    let geohash1 = String::from_slice(&env, "u4pruyd");
    let geohash2 = String::from_slice(&env, "u4pruyd");

    // Post two gists
    let gist_id1 = client.post_gist(&ipfs_cid1, &geohash1, &author1, &None);
    let gist_id2 = client.post_gist(&ipfs_cid2, &geohash2, &author2, &None);

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

    let gist = client.get_gist(&999);
    assert!(gist.is_none());
}

#[test]
fn test_get_gists_by_author() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    // Post gists from different authors
    client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    client.post_gist(&ipfs_cid, &geohash, &author2, &None);
    client.post_gist(&ipfs_cid, &geohash, &author1, &None);

    // Get gists by author1 with limit/offset
    let author1_gists = client.get_gists_by_author(&author1, &10u32, &0u32);
    assert_eq!(author1_gists.len(), 2);
}

#[test]
fn test_get_gists_by_author_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let other = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    // Post 5 gists from author, 1 from other
    for _ in 0..5 {
        client.post_gist(&ipfs_cid, &geohash, &author, &None);
    }
    client.post_gist(&ipfs_cid, &geohash, &other, &None);

    // First page: limit 2, offset 0 → gist ids 1, 2
    let page1 = client.get_gists_by_author(&author, &2u32, &0u32);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap(), 1u64);
    assert_eq!(page1.get(1).unwrap(), 2u64);

    // Second page: limit 2, offset 2 → gist ids 3, 4
    let page2 = client.get_gists_by_author(&author, &2u32, &2u32);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2.get(0).unwrap(), 3u64);
    assert_eq!(page2.get(1).unwrap(), 4u64);

    // Third page: limit 2, offset 4 → gist id 5 only
    let page3 = client.get_gists_by_author(&author, &2u32, &4u32);
    assert_eq!(page3.len(), 1);
    assert_eq!(page3.get(0).unwrap(), 5u64);
}

#[test]
#[should_panic(expected = "limit exceeds maximum of 50")]
fn test_get_gists_by_author_limit_exceeded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    client.get_gists_by_author(&author, &51u32, &0u32);
}

#[test]
fn test_get_active_gist_count() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let admin = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.initialize(&admin);

    let id1 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id2 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let _id3 = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    assert_eq!(client.get_active_gist_count(), 3);

    // Expire two gists
    client.expire_gist(&author, &id1);
    client.admin_expire_gist(&admin, &id2);

    assert_eq!(client.get_active_gist_count(), 1);
}

#[test]
fn test_get_gists_by_geohash() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    // Create test data
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash1 = String::from_slice(&env, "u4pruyd");
    let geohash2 = String::from_slice(&env, "u4pruyd");

    // Post gists with different geohashes
    client.post_gist(&ipfs_cid, &geohash1, &author, &None);
    client.post_gist(&ipfs_cid, &geohash2, &author, &None);

    // Get gists by geohash prefix
    let gists = client.get_gists_by_geohash(&String::from_slice(&env, "u4pruy"));
    assert_eq!(gists.len(), 2);
}

#[test]
fn test_custom_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let custom_expiry = env.ledger().timestamp() + 172800;

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &Some(custom_expiry));

    let gist = client.get_gist(&gist_id).unwrap();
    assert_eq!(gist.expiry, custom_expiry);
}

#[test]
fn test_post_gist_is_active() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let gist = client.get_gist(&gist_id).unwrap();
    assert!(gist.is_active);
}

#[test]
fn test_initialize_and_get_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
fn test_expire_gist_by_author() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert!(client.get_gist(&gist_id).unwrap().is_active);

    client.expire_gist(&author, &gist_id);

    let gist = client.get_gist(&gist_id).unwrap();
    assert!(!gist.is_active);
}

#[test]
#[should_panic(expected = "only the author can expire this gist")]
fn test_expire_gist_non_author_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let other = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.expire_gist(&other, &gist_id);
}

#[test]
fn test_admin_expire_gist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.initialize(&admin);
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    client.admin_expire_gist(&admin, &gist_id);

    let gist = client.get_gist(&gist_id).unwrap();
    assert!(!gist.is_active);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_admin_expire_gist_wrong_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.initialize(&admin);
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    client.admin_expire_gist(&impostor, &gist_id);
}

#[test]
fn test_batch_expire() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.initialize(&admin);
    let id1 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id2 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id3 = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let expired_count = client.batch_expire(&admin, &vec![&env, id1, id2, id3]);
    assert_eq!(expired_count, 3);

    assert!(!client.get_gist(&id1).unwrap().is_active);
    assert!(!client.get_gist(&id2).unwrap().is_active);
    assert!(!client.get_gist(&id3).unwrap().is_active);
}

#[test]
#[should_panic(expected = "batch size exceeds maximum of 20")]
fn test_batch_expire_exceeds_limit_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let ids = vec![
        &env, 1u64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ];
    client.batch_expire(&admin, &ids);
}

#[test]
fn test_set_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    client.set_admin(&admin, &new_admin);

    assert_eq!(client.get_admin(), Some(new_admin));
}

#[test]
#[should_panic(expected = "caller is not the current admin")]
fn test_set_admin_wrong_caller_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    client.set_admin(&impostor, &new_admin);
}

#[test]
fn test_is_gist_active_nonexistent() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    assert!(!client.is_gist_active(&999u64));
}

#[test]
fn test_is_gist_active_after_expire() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = the_gist_contracts::GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert!(client.is_gist_active(&gist_id));

    client.expire_gist(&author, &gist_id);
    assert!(!client.is_gist_active(&gist_id));
}
