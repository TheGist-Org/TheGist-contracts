use soroban_sdk::testutils::storage::{Instance as _, Temporary as _};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, String};
use the_gist_contracts::{GistRegistry, GistRegistryClient};

// ── initialize / admin ───────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    assert_eq!(client.get_gist_count(), 0);
}

#[test]
fn test_initialize_and_get_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
fn test_initialize_sets_version_to_1() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.get_version(), 1);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
fn test_set_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
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
    let client = GistRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.initialize(&admin);
    client.set_admin(&impostor, &new_admin);
}

// ── post_gist – happy path ───────────────────────────────────────────────────

#[test]
fn test_post_gist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert_eq!(gist_id, 1);
    assert_eq!(client.get_gist_count(), 1);

    let gist = client.get_gist(&gist_id).unwrap();
    assert_eq!(gist.gist_id, 1);
    assert_eq!(gist.ipfs_cid, ipfs_cid);
    assert_eq!(gist.geohash, geohash);
    assert_eq!(gist.author, author);
    assert!(gist.is_active);
}

#[test]
fn test_post_gist_uses_temporary_storage_and_instance_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest789");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.initialize(&admin);
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &Some(24));

    assert_eq!(client.get_version(), 1);
    env.as_contract(&contract_id, || {
        // Admin + ContractVersion + GistCount = 3 instance keys
        assert_eq!(env.storage().instance().all().len(), 3);
        // Gist + AuthorGists = 2 temporary keys
        assert_eq!(env.storage().temporary().all().len(), 2);
    });

    let gist = client.get_gist(&gist_id).unwrap();
    assert_eq!(gist.expiry, env.ledger().timestamp() + 86_400);
}

#[test]
fn test_post_gist_id_increments() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid1 = Bytes::from_slice(&env, b"QmTest123");
    let ipfs_cid2 = Bytes::from_slice(&env, b"QmTest456");
    let geohash = String::from_slice(&env, "u4pruyd");

    let id1 = client.post_gist(&ipfs_cid1, &geohash, &author1, &None);
    let id2 = client.post_gist(&ipfs_cid2, &geohash, &author2, &None);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(client.get_gist_count(), 2);
}

#[test]
fn test_post_gist_is_active() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert!(client.get_gist(&gist_id).unwrap().is_active);
}

#[test]
fn test_post_gist_custom_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let custom_expiry = env.ledger().timestamp() + 172800;
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &Some(custom_expiry));
    assert_eq!(client.get_gist(&gist_id).unwrap().expiry, custom_expiry);
}

// ── post_gist – validation failures ─────────────────────────────────────────

#[test]
#[should_panic(expected = "ipfs_cid cannot be empty")]
fn test_post_gist_empty_cid_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    let empty_cid = Bytes::from_slice(&env, b"");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.post_gist(&empty_cid, &geohash, &author, &None);
}

#[test]
#[should_panic(expected = "geohash must be exactly 7 characters")]
fn test_post_gist_geohash_too_short_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pru");
    client.post_gist(&ipfs_cid, &geohash, &author, &None);
}

#[test]
#[should_panic(expected = "geohash must be exactly 7 characters")]
fn test_post_gist_geohash_too_long_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruydx");
    client.post_gist(&ipfs_cid, &geohash, &author, &None);
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn test_post_gist_expiry_in_past_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let past_expiry = env.ledger().timestamp();
    client.post_gist(&ipfs_cid, &geohash, &author, &Some(past_expiry));
}

#[test]
#[should_panic(expected = "expiry cannot exceed 168 hours from now")]
fn test_post_gist_expiry_exceeds_max_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let far_future = env.ledger().timestamp() + 604801;
    client.post_gist(&ipfs_cid, &geohash, &author, &Some(far_future));
}

// ── get_gist ─────────────────────────────────────────────────────────────────

#[test]
fn test_get_gist_not_found() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    assert!(client.get_gist(&999).is_none());
}

// ── get_gists_by_author ───────────────────────────────────────────────────────

#[test]
fn test_get_gists_by_author() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    client.post_gist(&ipfs_cid, &geohash, &author2, &None);
    client.post_gist(&ipfs_cid, &geohash, &author1, &None);

    let result = client.get_gists_by_author(&author1, &10u32, &0u32);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_get_gists_by_author_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let other = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.post_gist(&ipfs_cid, &geohash, &other, &None);

    let result = client.get_gists_by_author(&author, &10u32, &0u32);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_extend_gist_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &Some(24));
    let original = client.get_gist(&gist_id).unwrap();

    client.extend_gist_ttl(&gist_id);

    let extended = client.get_gist(&gist_id).unwrap();
    assert_eq!(extended.expiry, original.expiry + 86_400);
    env.as_contract(&contract_id, || {
        assert_eq!(env.storage().instance().all().len(), 1);
        assert_eq!(env.storage().temporary().all().len(), 2);
    });
}

#[test]
#[should_panic]
fn test_extend_gist_ttl_requires_author_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &Some(24));

    env.set_auths(&[]);
    client.extend_gist_ttl(&gist_id);
}

#[test]
fn test_get_gists_by_author_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let other = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    for _ in 0..5 {
        client.post_gist(&ipfs_cid, &geohash, &author, &None);
    }
    client.post_gist(&ipfs_cid, &geohash, &other, &None);

    let page1 = client.get_gists_by_author(&author, &2u32, &0u32);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap(), 1u64);
    assert_eq!(page1.get(1).unwrap(), 2u64);

    let page2 = client.get_gists_by_author(&author, &2u32, &2u32);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2.get(0).unwrap(), 3u64);
    assert_eq!(page2.get(1).unwrap(), 4u64);

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
    let client = GistRegistryClient::new(&env, &contract_id);
    let author = Address::generate(&env);
    client.get_gists_by_author(&author, &51u32, &0u32);
}

// ── get_gists_by_geohash ─────────────────────────────────────────────────────

#[test]
fn test_get_gists_by_geohash() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");

    client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let gists = client.get_gists_by_geohash(&String::from_slice(&env, "u4pruy"));
    assert_eq!(gists.len(), 2);
}

// ── expire_gist ───────────────────────────────────────────────────────────────

#[test]
fn test_expire_gist_by_author() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    assert!(client.get_gist(&gist_id).unwrap().is_active);
    client.expire_gist(&author, &gist_id);
    assert!(!client.get_gist(&gist_id).unwrap().is_active);
}

#[test]
#[should_panic(expected = "only the author can expire this gist")]
fn test_expire_gist_non_author_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let other = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.expire_gist(&other, &gist_id);
}

// ── admin_expire_gist ────────────────────────────────────────────────────────

#[test]
fn test_admin_expire_gist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.initialize(&admin);
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    client.admin_expire_gist(&admin, &gist_id);
    assert!(!client.get_gist(&gist_id).unwrap().is_active);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_admin_expire_gist_wrong_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.initialize(&admin);
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.admin_expire_gist(&impostor, &gist_id);
}

// ── batch_expire ──────────────────────────────────────────────────────────────

#[test]
fn test_batch_expire() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.initialize(&admin);
    let id1 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id2 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id3 = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let expired = client.batch_expire(&admin, &vec![&env, id1, id2, id3]);
    assert_eq!(expired, 3);
    assert!(!client.get_gist(&id1).unwrap().is_active);
    assert!(!client.get_gist(&id2).unwrap().is_active);
    assert!(!client.get_gist(&id3).unwrap().is_active);
}

#[test]
fn test_batch_expire_skips_nonexistent() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.initialize(&admin);
    let id1 = client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let expired = client.batch_expire(&admin, &vec![&env, id1, 999u64]);
    assert_eq!(expired, 1);
    assert!(!client.get_gist(&id1).unwrap().is_active);
}

#[test]
#[should_panic(expected = "batch size exceeds maximum of 20")]
fn test_batch_expire_exceeds_limit_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    let ids = vec![
        &env, 1u64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ];
    client.batch_expire(&admin, &ids);
}

// ── is_gist_active ────────────────────────────────────────────────────────────

#[test]
fn test_is_gist_active_true_for_new_gist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert!(client.is_gist_active(&gist_id));
}

#[test]
fn test_is_gist_active_false_after_expire() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    let gist_id = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.expire_gist(&author, &gist_id);
    assert!(!client.is_gist_active(&gist_id));
}

#[test]
fn test_is_gist_active_false_for_nonexistent() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);
    assert!(!client.is_gist_active(&999u64));
}

// ── get_active_gist_count ─────────────────────────────────────────────────────

#[test]
fn test_get_active_gist_count() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, GistRegistry);
    let client = GistRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_slice(&env, "u4pruyd");
    client.initialize(&admin);

    let id1 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let id2 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let _id3 = client.post_gist(&ipfs_cid, &geohash, &author, &None);
    assert_eq!(client.get_active_gist_count(), 3);

    client.expire_gist(&author, &id1);
    client.admin_expire_gist(&admin, &id2);
    assert_eq!(client.get_active_gist_count(), 1);
}