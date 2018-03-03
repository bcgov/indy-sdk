extern crate rusqlcipher;
extern crate time;
extern crate hyper;
extern crate serde_json;
extern crate serde_derive;
extern crate reqwest;
extern crate indy_crypto;

use super::{Wallet, WalletType};

use errors::wallet::WalletError;
use utils::environment::EnvironmentUtils;
use hyper::header::{Headers};
use std::collections::HashMap;
use self::time::Timespec;
use utils::proxy;

use std::str;
use std::fs;
use std::path::PathBuf;

use self::indy_crypto::utils::json::JsonDecodable;

/*
 * Implementation of a remote/virtual wallet store.
 * 
 * This wallet can store claims and other information for multiple subjects, and is
 * intended to support the use case where an organization has to manage claims an other
 * data for a large number of organizations or subjects.
 * 
 * This wallet supports a "root" wallet (for credentials and other info shared across
 * all subjects, or referring to the managing organization), and multiple "virtual" 
 * wallets, one per subject.
 * 
 * The virtual wallet name is specifid in the wallet credentials:
 * 
 *     {auth_token: "Token <your DRF token>", virtual_wallet: "subject1_wallet"}
 * 
 * If the virtual_wallet is not specified then the wallet name is used as the virtual wallet.
 * 
 * This code is cloned from "default.rs" and an additional database column added to the
 * wallet database to specify the virtual wallet.
 * 
 * Key names can be duplicated across virtual wallets.
 */

#[derive(Deserialize, Serialize)]
struct RemoteWalletRuntimeConfig {
    endpoint: String,
    freshness_time: i64
}

impl<'a> JsonDecodable<'a> for RemoteWalletRuntimeConfig {}

impl Default for RemoteWalletRuntimeConfig {
    fn default() -> Self {
        RemoteWalletRuntimeConfig { 
            endpoint: String::from("http://localhost:8000/items/"), 
            freshness_time: 1000 
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct RemoteWalletCredentials {
    auth_token: Option<String>,      // "Token <token>" or "JWT <token>" for example
    virtual_wallet: Option<String>   // virtual wallet name (optional)
}

impl<'a> JsonDecodable<'a> for RemoteWalletCredentials {}

impl Default for RemoteWalletCredentials {
    fn default() -> Self {
        RemoteWalletCredentials { 
            auth_token: None, 
            virtual_wallet: None 
        }
    }
}

struct RemoteWalletRecord {
    wallet_name: String,
    key_type: String,
    key: String,
    value: String,
    time_created: Timespec
}

struct RemoteWallet {
    wallet_name: String,
    pool_name: String,
    config: RemoteWalletRuntimeConfig,
    credentials: RemoteWalletCredentials
}

impl RemoteWallet {
    fn new(name: &str,
           pool_name: &str,
           config: RemoteWalletRuntimeConfig,
           credentials: RemoteWalletCredentials) -> RemoteWallet {
        RemoteWallet {
            wallet_name: name.to_string(),
            pool_name: pool_name.to_string(),
            config: config,
            credentials: credentials
        }
    }
}

// Helper function for the root wallet name 
fn root_wallet_name(wallet_name: &str) -> String {
    wallet_name.to_string()
}

// Helper function to extract the virtual wallet name from the credentials
fn virtual_wallet_name(wallet_name: &str, credentials: &RemoteWalletCredentials) -> String {
    match credentials.virtual_wallet {
        Some(ref s) => s.to_string(),
        None => wallet_name.to_string()
    }
}

// Helper function to construct the endpoint for a REST request
fn rest_endpoint(config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials, 
                    wallet_name: &str) -> String {
    proxy::rest_endpoint(&config.endpoint, Some(&virtual_wallet_name(wallet_name, credentials)))
}

// Helper function to construct the endpoint for a REST request
fn rest_endpoint_for_set(config: &RemoteWalletRuntimeConfig, 
                        credentials: &RemoteWalletCredentials) -> String {
    proxy::rest_endpoint(&config.endpoint, None)
}

// Helper function to construct the endpoint for a REST request for a specific resource (wallet item)
fn rest_endpoint_for_resource(config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials, 
                    item_wallet_name: &str, item_key: &str) -> String {
    let endpoint = proxy::rest_endpoint(&config.endpoint, Some(item_wallet_name));
    proxy::rest_resource_endpoint(&endpoint, item_key)
}

// Helper function to construct the endpoint for a REST request for a specific resource (wallet item)
fn rest_endpoint_for_resource_id(config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials, 
                    item_wallet_name: &str, item_type: &str, item_id: &str) -> String {
    let endpoint = proxy::rest_endpoint(&config.endpoint, Some(item_wallet_name));
    let endpoint = proxy::rest_resource_endpoint(&endpoint, item_type);
    proxy::rest_resource_endpoint(&endpoint, item_id)
}

// Helper function to onstruct the AUTH header for a REST request
fn rest_auth_header(config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials) -> Headers {
    let in_token = credentials.auth_token.clone();
    match in_token {
        Some(s) => proxy::rest_auth_headers(&s),
        None => panic!("Error no authentication token provided")
    }
}

// helper function to convert "key" to "item_type" and "item_id"
fn key_to_item_type_id(key: &str) -> (String, String) {
    let split = key.split("::");
    let vec: Vec<&str> = split.collect();
    if vec.len() == 2 {
        (vec[0].to_owned(), vec[1].to_owned())
    } else {
        panic!(format!("Error invalid key {}", key));
    }
}

// helper function to convert "item_type" and "item_id" to "key"
fn item_type_id_to_key(item_type: &str, item_id: &str) -> String {
    let mut key = item_type.to_owned();
    key.push_str(&"::".to_owned());
    key.push_str(&item_id.to_owned());
    key
}

// helper function to convert key profix to item type (basically removes the "::")
fn key_prefix_to_type(key_prefix: &str) -> String {
    let split = key_prefix.split("::");
    let vec: Vec<&str> = split.collect();
    if vec.len() > 0 {
        vec[0].to_owned()
    } else {
        panic!(format!("Error invalid format on key prefix {}", key_prefix));
    }
}

// Helper method to fetch claims
// wallet_name is either the virtual wallet or the root wallet
fn call_get_internal(root_wallet_name: &str, wallet_name: &str,
                    config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials, 
                    key: &str) -> Result<String, WalletError> {
    
    let (item_type, item_id) = key_to_item_type_id(key);
    
    // build request URL
    let req_url = rest_endpoint_for_resource_id(config, credentials, wallet_name, &item_type, &item_id);

    // build auth headers
    let headers = rest_auth_header(config, credentials);

    // build REST request and execute
    let response = proxy::rest_get_request(&req_url, Some(headers));
    match response {
        Ok(r) => {
            let result = proxy::rest_extract_response_body(r);
            match result {
                Ok(s) => Ok(s),
                Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
            }
        },
        Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
    }
} 

impl Wallet for RemoteWallet {
    fn set(&self, key: &str, value: &str) -> Result<(), WalletError> {
        let (item_type, item_id) = key_to_item_type_id(key);

        // build request URL
        let req_url = rest_endpoint_for_set(&self.config, &self.credentials);

        // build auth headers
        let headers = rest_auth_header(&self.config, &self.credentials);

        // build payload
        let wallet_name = &virtual_wallet_name(&self.wallet_name, &self.credentials)[..];
        let mut map = HashMap::new();
        map.insert("wallet_name", wallet_name);
        map.insert("item_type", &item_type);
        map.insert("item_id", &item_id);
        map.insert("item_value", value);

        // build REST request and execute
        let response = proxy::rest_post_request_map(&req_url, Some(headers), Some(&map));
        match response {
            Ok(r) => {
                let result = proxy::rest_check_result(r);
                match result {
                    Ok(()) => Ok(()),
                    Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
                }
            },
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }
    }

    // get will first check the selected wallet, and if the key is not found, 
    // will *also* check the root wallet
    // keys shared between all virtual wallets can be stored once in the root
    fn get(&self, key: &str) -> Result<String, WalletError> {
        let result = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                        &virtual_wallet_name(&self.wallet_name, &self.credentials),
                                        &self.config, &self.credentials, key);
        match result {
            Ok(record) => Ok(record),
            Err(why) => {
                let result2 = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                                &root_wallet_name(&self.wallet_name),
                                                &self.config, &self.credentials, key);
                match result2 {
                    Ok(record2) => Ok(record2),
                    Err(why2) => Err(WalletError::NotFound(format!("{:?}", why2)))
                }
            }
        }
    }

    // list will return records only from the selected wallet (root or virtual)
    fn list(&self, key_prefix: &str) -> Result<Vec<(String, String)>, WalletError> {
        let item_type = key_prefix_to_type(key_prefix);

        // build request URL
        let req_url = rest_endpoint_for_resource(&self.config, &self.credentials, 
                        &virtual_wallet_name(&self.wallet_name, &self.credentials), &item_type);

        // build auth headers
        let headers = rest_auth_header(&self.config, &self.credentials);

        // build REST request and execute
        let response = proxy::rest_get_request(&req_url, Some(headers));
        match response {
            Ok(r) => {
                let result = proxy::rest_extract_response_body(r);
                match result {
                    Ok(s) => {
                        // TODO parse the result string into an array of items
                        let mut key_values = Vec::new();
                        Ok(key_values)
                    },
                    Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
                }
            },
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }

        /*
        let connection = _open_connection(root_wallet_name(&self.wallet_name).as_str(), &self.credentials)?;
        let mut stmt = connection.prepare("SELECT key, value, time_created 
                FROM wallet WHERE virtual_wallet = ?1 AND key like ?2 order by key")?;
        let records = stmt.query_map(&[&virtual_wallet_name(&self.wallet_name, &self.credentials).as_str(), &format!("{}%", key_prefix)], |row| {
            RemoteWalletRecord {
                wallet_name: "".to_owned(),
                key_type: "".to_owned(),
                key: row.get(0),
                value: row.get(1),
                time_created: row.get(2)
            }
        })?;
        */

        //let mut key_values = Vec::new();

        // TODO loop through response and build array to return
        //for record in records {
        //    let key_value = record?;
        //    key_values.push((key_value.key, key_value.value));
        //}

        //Ok(key_values)
    }

    // TODO get_not_expired will first check the selected wallet, and if the key is not found, 
    // will *also* check the root wallet
    // keys shared between all virtual wallets can be stored once in the root
    fn get_not_expired(&self, key: &str) -> Result<String, WalletError> {
        let (item_type, item_id) = key_to_item_type_id(key);
        
        // build request URL
        let req_url = rest_endpoint_for_resource_id(&self.config, &self.credentials, 
                        &virtual_wallet_name(&self.wallet_name, &self.credentials), 
                        &item_type, &item_id);

        // build auth headers
        let headers = rest_auth_header(&self.config, &self.credentials);

        // build REST request and execute
        let response = proxy::rest_get_request(&req_url, Some(headers));
        match response {
            Ok(r) => {
                let result = proxy::rest_extract_response_body(r);
                // TODO need to do the validation magic around te expiry time
                match result {
                    Ok(s) => Ok(s),
                    Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
                }
            },
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }

        /*
        let record = _open_connection(root_wallet_name(&self.wallet_name).as_str(), &self.credentials)?
            .query_row(
                "SELECT key, value, time_created 
                FROM wallet WHERE virtual_wallet = ?1 AND key = ?2 LIMIT 1",
                &[&virtual_wallet_name(&self.wallet_name, &self.credentials).as_str(), &key.to_string()], |row| {
                    RemoteWalletRecord {
                        wallet_name: "".to_owned(),
                        key_type: "".to_owned(),
                        key: row.get(0),
                        value: row.get(1),
                        time_created: row.get(2)
                    }
                })?;
        */

        /*
        let record = RemoteWalletRecord {
                        wallet_name: "".to_owned(),
                        key_type: "".to_owned(),
                        key: "".to_owned(),
                        value: "".to_owned(),
                        time_created: Timespec::new(60,0)
                    };

        if self.config.freshness_time != 0
            && time::get_time().sub(record.time_created).num_seconds() > self.config.freshness_time {
            return Err(WalletError::NotFound(key.to_string()));
        }

        return Ok(record.value);
        */
    }

    fn close(&self) -> Result<(), WalletError> { Ok(()) }

    fn get_pool_name(&self) -> String {
        self.pool_name.clone()
    }

    fn get_name(&self) -> String {
        self.wallet_name.clone()
    }
}

pub struct RemoteWalletType {}

impl RemoteWalletType {
    pub fn new() -> RemoteWalletType {
        RemoteWalletType {}
    }
}

impl WalletType for RemoteWalletType {
    fn create(&self, name: &str, config: Option<&str>, credentials: Option<&str>) -> Result<(), WalletError> {
        trace!("RemoteWalletType.create >> {}, with config {:?} and credentials {:?}", name, config, credentials);
        let root_name = root_wallet_name(&name);
        let path = _db_path(&root_name);
        if path.exists() {
            trace!("RemoteWalletType.create << path exists");
            return Err(WalletError::AlreadyExists(root_name.to_string()));
        }

        let runtime_config = match config {
            Some(config) => RemoteWalletRuntimeConfig::from_json(config)?,
            None => RemoteWalletRuntimeConfig::default()
        };

        let runtime_auth = match credentials {
            Some(auth) => RemoteWalletCredentials::from_json(auth)?,
            None => RemoteWalletCredentials::default()
        };

        // the wallet should exist, let's try to ping the server and make sure it exists(?)
        // we'll do a schema request, verify that it returns an "OK" status
        let endpoint = proxy::rest_append_path("http://localhost:8000/", "schema");
        let response = proxy::rest_get_request(&endpoint, None);
        match response {
            Ok(r) => {
                let result = proxy::rest_check_result(r);
                match result {
                    Ok(()) => {
                        // TODO authenticate and cache our token (?)
                        trace!("RemoteWalletType.create <<");
                        Ok(())
                    },
                    Err(e) => Err(WalletError::AccessFailed(format!("{:?}", e)))
                }
            }
            Err(e) => Err(WalletError::AccessFailed(format!("{:?}", e)))
        }
    }

    fn delete(&self, name: &str, config: Option<&str>, credentials: Option<&str>) -> Result<(), WalletError> {
        trace!("RemoteWalletType.delete {}, with config {:?} and credentials {:?}", name, config, credentials);
        // FIXME: parse and implement credentials!!!
        let root_name = root_wallet_name(&name);
        Ok(fs::remove_file(_db_path(&root_name)).map_err(map_err_trace!())?)
    }

    fn open(&self, name: &str, pool_name: &str, config: Option<&str>, runtime_config: Option<&str>, credentials: Option<&str>) -> Result<Box<Wallet>, WalletError> {
        let runtime_config = match runtime_config {
            Some(config) => RemoteWalletRuntimeConfig::from_json(config)?,
            None => RemoteWalletRuntimeConfig::default()
        };

        let runtime_auth = match credentials {
            Some(auth) => RemoteWalletCredentials::from_json(auth)?,
            None => RemoteWalletCredentials::default()
        };

        let root_name = root_wallet_name(&name);

        // we'll do a schema request, verify that it returns an "OK" status
        let endpoint = proxy::rest_append_path("http://localhost:8000/", "schema");
        let response = proxy::rest_get_request(&endpoint, None);
        match response {
            Ok(r) => {
                let result = proxy::rest_check_result(r);
                match result {
                    Ok(()) => {
                        // TODO authenticate and cache our token (?)
                        trace!("RemoteWalletType.create <<");

                        Ok(Box::new(
                            RemoteWallet::new(
                                name,
                                pool_name,
                                runtime_config,
                                runtime_auth)))
                    },
                    Err(e) => Err(WalletError::AccessFailed(format!("{:?}", e)))
                }
            }
            Err(e) => Err(WalletError::AccessFailed(format!("{:?}", e)))
        }
    }
}

fn _db_path(name: &str) -> PathBuf {
    let mut path = EnvironmentUtils::wallet_path(name);
    path.push("sqlite.db");
    path
}


#[cfg(test)]
mod tests {
    use super::*;
    use errors::wallet::WalletError;
    use utils::test::TestUtils;

    use serde_json;
    use self::serde_json::Error as JsonError;

    use std::time::Duration;
    use std::thread;

    #[test]
    fn virtual_wallet_name_works() {
        let w1 = root_wallet_name("root");
        assert_eq!("root", w1);
        
        let credentials1 = RemoteWalletCredentials{auth_token: Some(String::from("Token 1234567890")), 
                            virtual_wallet: Some(String::from("virtual"))};
        let w2 = virtual_wallet_name("root", &credentials1);
        assert_eq!("virtual", w2);
        
        let w3 = root_wallet_name("root");
        assert_eq!("root", w3);
        
        let credentials2 = RemoteWalletCredentials{auth_token: Some(String::from("Token 1234567890")), 
                            virtual_wallet: None};
        let w4 = virtual_wallet_name("root", &credentials2);
        assert_eq!("root", w4);
    }

    #[test]
    fn remote_wallet_type_new_works() {
        RemoteWalletType::new();
    }

    #[test]
    fn remote_wallet_type_create_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_type_create_works_for_twice() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let res = wallet_type.create("wallet1", None, None);
        assert_match!(Err(WalletError::AlreadyExists(_)), res);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_type_delete_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        wallet_type.delete("wallet1", None, None).unwrap();
        wallet_type.create("wallet1", None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_type_open_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_virtual_wallet_type_open_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"auth_token":"","virtual_wallet":"some1"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();

        let credentials2 = Some(r#"{"auth_token":"","virtual_wallet":"some2"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();

        let credentials3 = Some(r#"{"auth_token":"","virtual_wallet":"some3"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials3).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_base() {
        TestUtils::cleanup_indy_home();

        // set configuraiton, including endpoint
        let config = RemoteWalletRuntimeConfig::default();

        let auth_endpoint = proxy::rest_endpoint("http://localhost:8000/", Some("api-token-auth"));
        let response = proxy::rest_post_request_auth(&auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => {     // ok, returned a token, try the "GET" again
                let token = s;

                // build credentials before creating and opening wallet
                let auth_creds = RemoteWalletCredentials { 
                    auth_token: Some(token), 
                    virtual_wallet: None 
                };

                let cf_str = serde_json::to_string(&config).unwrap();
                let ac_str = serde_json::to_string(&auth_creds).unwrap();

                let wallet_type = RemoteWalletType::new();
                wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
                let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();

                let result = wallet.get("type::key1");
                match result {
                    Ok(s) => print!("Reurned value {}", s),
                    Err(e) => print!("Error {:?}", e)
                };
                wallet.set("type::key1", "value1").unwrap();
                let value = wallet.get("type::key1").unwrap();
                assert_eq!("value1", value);

            },
            Err(e) => assert!(false, format!("{:?}", e))
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_virtual_wallet_set_get_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"auth_token":"","virtual_wallet":"some1"}"#);
        let credentials2 = Some(r#"{"auth_token":"","virtual_wallet":"some2"}"#);
        let credentials3 = Some(r#"{"auth_token":"","virtual_wallet":"some3"}"#);

        {
            let wallet1 = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();
            wallet1.set("type::key1", "value1").unwrap();
            let value1 = wallet1.get("type::key1").unwrap();
            assert_eq!("value1", value1);
        }

        {
            let wallet2 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            wallet2.set("type::key1", "value2").unwrap();
            let value2 = wallet2.get("type::key1").unwrap();
            assert_eq!("value2", value2);
        }

        {
            let wallet3 = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();
            let value3 = wallet3.get("type::key1").unwrap();
            assert_eq!("value1", value3);
        }

        {
            let wallet4 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet4.set("key1", "value_root").unwrap();
            let value4 = wallet4.get("type::key1").unwrap();
            assert_eq!("value_root", value4);
        }

        {
            let wallet5 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            let value5 = wallet5.get("type::key1").unwrap();
            assert_eq!("value2", value5);
        }

        {
            let wallet6 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            let value6 = wallet6.get("type::key1").unwrap();
            assert_eq!("value_root", value6);
        }

        // create key in root and fetch in virtual wallet
        {
            let wallet7 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet7.set("type::root_only_key", "value_root_only").unwrap();
            let value7 = wallet7.get("type::root_only_key").unwrap();
            assert_eq!("value_root_only", value7);
        }
        {
            let wallet8 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            let value8 = wallet8.get("type::root_only_key").unwrap();
            assert_eq!("value_root_only", value8);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_reopen() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet.set("type::key1", "value1").unwrap();
        }

        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
        let value = wallet.get("type::key1").unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_get_works_for_unknown() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
        let value = wallet.get("type::key1");
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_update() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        wallet.set("type::key1", "value1").unwrap();
        let value = wallet.get("type::key1").unwrap();
        assert_eq!("value1", value);

        wallet.set("type::key1", "value2").unwrap();
        let value = wallet.get("type::key1").unwrap();
        assert_eq!("value2", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_not_expired_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some("{\"freshness_time\": 1}"), None).unwrap();
        wallet.set("type::key1", "value1").unwrap();

        // Wait until value expires
        thread::sleep(Duration::new(2, 0));

        let value = wallet.get_not_expired("type::key1");
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        wallet.set("key1::subkey1", "value1").unwrap();
        wallet.set("key1::subkey2", "value2").unwrap();

        let mut key_values = wallet.list("key1::").unwrap();
        key_values.sort();
        assert_eq!(2, key_values.len());

        let (key, value) = key_values.pop().unwrap();
        assert_eq!("key1::subkey2", key);
        assert_eq!("value2", value);

        let (key, value) = key_values.pop().unwrap();
        assert_eq!("key1::subkey1", key);
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"auth_token":"","virtual_wallet":"client1"}"#);
        let credentials2 = Some(r#"{"auth_token":"","virtual_wallet":"client2"}"#);

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();

            wallet.set("key1::subkey1", "value1").unwrap();
            wallet.set("key1::subkey2", "value2").unwrap();
        }

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();

            wallet.set("key1::subkey1", "value3").unwrap();
            wallet.set("key1::subkey2", "value4").unwrap();
            wallet.set("key1::subkey3", "value5").unwrap();
        }

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();

            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value1", value);
        }

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();

            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(3, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey3", key);
            assert_eq!("value5", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value4", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value3", value);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_get_pool_name_works() {
        TestUtils::cleanup_indy_home();

        let remote_wallet_type = RemoteWalletType::new();
        remote_wallet_type.create("wallet1", None, None).unwrap();
        let wallet = remote_wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        assert_eq!(wallet.get_pool_name(), "pool1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_get_name_works() {
        TestUtils::cleanup_indy_home();

        let remote_wallet_type = RemoteWalletType::new();
        remote_wallet_type.create("wallet1", None, None).unwrap();
        let wallet = remote_wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        assert_eq!(wallet.get_name(), "wallet1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_credentials_deserialize() {
        let empty: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{}"#);
        assert!(empty.is_err());

        let one: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{"auth_token":""}"#);
        assert!(one.is_ok());
        let rone = one.unwrap();
        assert_eq!(rone.auth_token, Some("".to_string()));
        assert_eq!(rone.virtual_wallet, None);

        let two: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{"auth_token":"thisisatest","virtual_wallet":null}"#);
        assert!(two.is_ok());
        let rtwo = two.unwrap();
        assert_eq!(rtwo.auth_token, Some("thisisatest".to_string()));
        assert_eq!(rtwo.virtual_wallet, None);

        let three: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{"auth_token":"","virtual_wallet:"thisismynewpassword"}"#);
        assert!(three.is_ok());
        let rthree = three.unwrap();
        assert_eq!(rthree.auth_token, Some("".to_string()));
        assert_eq!(rthree.virtual_wallet, Some("thisismynewpassword".to_string()));

        let four: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{"auth_token": "", "virtual_wallet": ""}"#);
        assert!(four.is_ok());
        let rfour = four.unwrap();
        assert_eq!(rfour.auth_token, Some("".to_string()));
        assert_eq!(rfour.virtual_wallet, Some("".to_string()));
    }

    #[test]
    fn remote_wallet_convert_nonencrypted_to_encrypted() {
        TestUtils::cleanup_indy_home();
        {
            let remote_wallet_type = RemoteWalletType::new();
            remote_wallet_type.create("mywallet", None, Some(r#"{"auth_token":""}"#)).unwrap();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"auth_token":""}"#)).unwrap();

            wallet.set("key1::subkey1", "value1").unwrap();
            wallet.set("key1::subkey2", "value2").unwrap();
        }
        {
            let remote_wallet_type = RemoteWalletType::new();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"auth_token":"", "virtual_wallet":"thisisatest"}"#)).unwrap();
            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value1", value);
        }
        {
            let remote_wallet_type = RemoteWalletType::new();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":"thisisatest"}"#)).unwrap();

            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value1", value);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_convert_encrypted_to_nonencrypted() {
        TestUtils::cleanup_indy_home();
        {
            let remote_wallet_type = RemoteWalletType::new();
            remote_wallet_type.create("mywallet", None, Some(r#"{"auth_token":"thisisatest"}"#)).unwrap();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"auth_token":"thisisatest"}"#)).unwrap();

            wallet.set("key1::subkey1", "value1").unwrap();
            wallet.set("key1::subkey2", "value2").unwrap();
        }
        {
            let remote_wallet_type = RemoteWalletType::new();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"auth_token":"thisisatest", "virtual_wallet":""}"#)).unwrap();
            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value1", value);
        }
        {
            let remote_wallet_type = RemoteWalletType::new();
            let wallet = remote_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":""}"#)).unwrap();

            let mut key_values = wallet.list("key1::").unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey2", key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!("key1::subkey1", key);
            assert_eq!("value1", value);
        }

        TestUtils::cleanup_indy_home();
    }
}
