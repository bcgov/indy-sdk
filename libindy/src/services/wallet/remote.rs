extern crate rusqlcipher;
extern crate time;
extern crate hyper;
extern crate serde_json;
extern crate reqwest;
extern crate indy_crypto;
extern crate rand;

use super::{Wallet, WalletType};

use errors::common::CommonError;
use errors::wallet::WalletError;
use utils::environment::EnvironmentUtils;
use hyper::header::{Headers};
use std::collections::HashMap;
use self::time::Timespec;
use utils::proxy;

use std::str;
use std::path::PathBuf;
use std::ops::Sub;

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

// helper functio to check if we are in a virtual wallet or root
fn in_virtual_wallet(root_wallet_name: &str, virtual_wallet_name: &str) -> bool {
    root_wallet_name != virtual_wallet_name
}

// Helper function to construct the endpoint for a REST request
fn rest_endpoint(config: &RemoteWalletRuntimeConfig, 
                    credentials: &RemoteWalletCredentials, 
                    wallet_name: &str) -> String {
    proxy::rest_endpoint(&config.endpoint, Some(&virtual_wallet_name(wallet_name, credentials)))
}

// Helper function to construct the endpoint for a REST request
fn rest_endpoint_for_set(config: &RemoteWalletRuntimeConfig, 
                        credentials: &RemoteWalletCredentials,
                        id: Option<&str>) -> String {
    proxy::rest_endpoint(&config.endpoint, id)
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
                    key: &str) -> Result<(String, String), WalletError> {
    
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
                Ok(s) => {
                    let hm = rest_body_to_items(&s);
                    match hm {
                        Ok(v) => {
                            if v.len() == 0 {
                                Err(WalletError::NotFound(format!("Error Item not found")))
                            } else if v.len() > 1 {
                                Err(WalletError::NotFound(format!("Error Multiple Items found")))
                            } else {
                                Ok((v[0]["id"].to_owned(), v[0]["item_value"].to_owned()))
                            }
                        },
                        Err(e) => Err(e)
                    }
                },
                Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
            }
        },
        Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
    }
} 

pub fn rest_body_to_items(body: &str) -> Result<Vec<HashMap<String, String>>, WalletError> {
    let mut keys: Vec<&str> = Vec::new();
    keys.push("wallet_name");
    keys.push("item_type");
    keys.push("item_id");
    keys.push("item_value");
    keys.push("id");
    keys.push("created");
    let objects = proxy::body_as_vec(body, &keys);
    match objects {
        Ok(s) => Ok(s),
        Err(e) => Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid response from wallet"))))
    }
}

impl Wallet for RemoteWallet {
    fn set(&self, key: &str, value: &str) -> Result<(), WalletError> {
        let (item_type, item_id) = key_to_item_type_id(key);

        // check if we are doing  create or update
        let result = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                        &virtual_wallet_name(&self.wallet_name, &self.credentials),
                                        &self.config, &self.credentials, key);
        let tmp_id = match result {
            Ok((id, _s)) => id,
            Err(_e) => "".to_owned()
        };
        //println!("Found existing id (?) {}", tmp_id);
        let tmp2_id: &str = &tmp_id[..];
        let set_id = if tmp_id.len() > 0 {
            //println!("Sending id {}", tmp_id);
            Some(tmp2_id)
        } else {
            //println!("Sending None id");
            None
        };
        
        // build request URL
        let req_url = rest_endpoint_for_set(&self.config, &self.credentials, set_id);
        //println!("Sending to URL {}", req_url);

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
        let response;
        if tmp_id.len() > 0 {
            response = proxy::rest_put_request_map(&req_url, Some(headers), Some(&map));
        } else {
            response = proxy::rest_post_request_map(&req_url, Some(headers), Some(&map));
        }
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
            Ok((_s, record)) => Ok(record),
            Err(why) => {
                if in_virtual_wallet(&root_wallet_name(&self.wallet_name), 
                                        &virtual_wallet_name(&self.wallet_name, &self.credentials)) {
                    let result2 = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                                    &root_wallet_name(&self.wallet_name),
                                                    &self.config, &self.credentials, key);
                    match result2 {
                        Ok((_s, record2)) => Ok(record2),
                        Err(why2) => Err(WalletError::NotFound(format!("{:?}", why2)))
                    }
                } else {
                    Err(WalletError::NotFound(format!("{:?}", why)))
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
                        // parse the result string into an array of items
                        let hm = rest_body_to_items(&s);
                        match hm {
                            Ok(v) => {
                                let mut key_values = Vec::new();

                                // loop through response and build array to return
                                for record in v {
                                    let mut item_type = record["item_type"].to_owned();
                                    item_type.push_str("::");
                                    item_type.push_str(&record["item_id"]);
                                    let item_value = record["item_value"].to_owned();
                                    key_values.push((item_type, item_value));
                                }

                                Ok(key_values)
                            },
                            Err(e) => Err(e)
                        }
                    },
                    Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
                }
            },
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }
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
                match result {
                    Ok(s) => {
                        // parse the result string into an array of items
                        let hm = rest_body_to_items(&s);
                        match hm {
                            Ok(v) => {
                                if v.len() == 0 {
                                    Err(WalletError::NotFound(format!("Error Item not found")))
                                } else if v.len() > 1 {
                                    Err(WalletError::NotFound(format!("Error Multiple Items found")))
                                } else {
                                    // do the validation magic around the expiry time
                                    let time_created = time::strptime(&v[0]["created"], "%Y-%m-%dT%H:%M:%S").unwrap();
                                    if self.config.freshness_time != 0
                                        && time::now_utc().sub(time_created).num_seconds() > self.config.freshness_time {
                                        return Err(WalletError::NotFound(key.to_string()));
                                    }
                                    Ok(v[0]["item_value"].to_owned())
                                }
                            },
                            Err(e) => Err(e)
                        }
                    },
                    Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
                }
            },
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }
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
        //Ok(fs::remove_file(_db_path(&root_name)).map_err(map_err_trace!())?)
        // this is a no-op for the remote wallet
        Ok(())
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
    use rand::{thread_rng, Rng};

    fn rand_type(type_prefix: &str) -> String {
        let mut rng = thread_rng();
        let num: i32 = rng.gen_range(0, 999999);
        let snum: String = format!("{:06}", num);
        let mut my_type: String = "".to_owned();
        my_type.push_str(type_prefix);
        my_type.push_str(&snum);
        my_type
    }

    fn rand_key(key_type: &str, key_prefix: &str) -> String {
        let mut rng = thread_rng();
        let num: i32 = rng.gen_range(0, 999999);
        let snum: String = format!("{:06}", num);
        let mut my_id: String = "".to_owned();
        my_id.push_str(key_type);
        my_id.push_str("::");
        my_id.push_str(key_prefix);
        my_id.push_str(&snum);
        my_id
    }

    fn default_config_for_test() -> String {
        let config = RemoteWalletRuntimeConfig::default();
        let cf_str = serde_json::to_string(&config).unwrap();
        cf_str
    }

    fn default_credentials_for_test(token: &str) -> String {
        // build credentials before creating and opening wallet
        let auth_creds = RemoteWalletCredentials { 
            auth_token: Some(token.to_owned()), 
            virtual_wallet: None 
        };
        let ac_str = serde_json::to_string(&auth_creds).unwrap();
        ac_str
    }

    fn default_virtual_credentials_for_test(token: &str, virtual_wallet: &str) -> String {
        // build credentials before creating and opening wallet
        let auth_creds = RemoteWalletCredentials { 
            auth_token: Some(token.to_owned()), 
            virtual_wallet: Some(virtual_wallet.to_owned())
        };
        let ac_str = serde_json::to_string(&auth_creds).unwrap();
        ac_str        
    }

    fn verify_rest_server() -> String {
        let auth_endpoint = proxy::rest_endpoint("http://localhost:8000/", Some("api-token-auth"));
        let response = proxy::rest_post_request_auth(&auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => s,
            Err(e) => {
                assert!(false, format!("{:?}", e));
                "".to_owned()
            }
        }
    }

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
    fn in_virtual_wallet_works() {
        assert!(in_virtual_wallet("root_wallet", "virtual_wallet"));
        assert!(!in_virtual_wallet("root_wallet", "root_wallet"));
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
        // create works twice for rest remote virtual wallet
        //assert_match!(Err(WalletError::AlreadyExists(_)), res);
        assert_match!(Ok(()), res);

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
    fn virtual_wallet_set_get_works_base1() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();

        let my_id = rand_key("type", "key_");
        let result = wallet.get(&my_id);
        match result {
            Ok(s) => println!("Reurned value {}", s),
            Err(e) => println!("Error {:?}", e)
        };
        wallet.set(&my_id, "value1").unwrap();
        let value = wallet.get(&my_id).unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_base2() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();

        let my_id = rand_key("type", "key_");
        let result = wallet.get(&my_id);
        match result {
            Ok(s) => println!("Reurned value {}", s),
            Err(e) => println!("Error {:?}", e)
        };
        wallet.set(&my_id, "value1").unwrap();
        let value = wallet.get(&my_id).unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_virtual_wallet_set_get_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();

        let credentials1 = default_virtual_credentials_for_test(&token, "some1");
        let my_key1 = rand_key("type", "key_");
        let credentials2 = default_virtual_credentials_for_test(&token, "some2");
        let my_key2 = rand_key("type", "key_");
        let credentials3 = default_virtual_credentials_for_test(&token, "some3");
        let my_key3 = rand_key("type", "key_");
        let my_key4 = rand_key("type", "root_");

        {
            let wallet1 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials1)).unwrap();
            wallet1.set(&my_key1, "value1").unwrap();
            let value1 = wallet1.get(&my_key1).unwrap();
            assert_eq!("value1", value1);
        }

        {
            let wallet2 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials2)).unwrap();
            wallet2.set(&my_key2, "value2").unwrap();
            let value2 = wallet2.get(&my_key2).unwrap();
            assert_eq!("value2", value2);
        }

        {
            let wallet3 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials1)).unwrap();
            let value3 = wallet3.get(&my_key1).unwrap();
            assert_eq!("value1", value3);
        }

        {
            let wallet4 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
            wallet4.set(&my_key3, "value_root").unwrap();
            let value4 = wallet4.get(&my_key3).unwrap();
            assert_eq!("value_root", value4);
        }

        {
            let wallet5 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials2)).unwrap();
            let value5 = wallet5.get(&my_key2).unwrap();
            assert_eq!("value2", value5);
        }

        {
            let wallet6 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
            let value6 = wallet6.get(&my_key3).unwrap();
            assert_eq!("value_root", value6);
        }

        // create key in root and fetch in virtual wallet
        {
            let wallet7 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
            wallet7.set(&my_key4, "value_root_only").unwrap();
            let value7 = wallet7.get(&my_key4).unwrap();
            assert_eq!("value_root_only", value7);
        }
        {
            let wallet8 = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials2)).unwrap();
            let value8 = wallet8.get(&my_key4).unwrap();
            assert_eq!("value_root_only", value8);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_reopen() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let my_key1 = rand_key("type", "key_");

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
            wallet.set(&my_key1, "value1").unwrap();
        }

        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
        let value = wallet.get(&my_key1).unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_get_works_for_unknown() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let my_key1 = rand_key("type", "key_");

        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
        let value = wallet.get(&my_key1);
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_update() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
        let my_key1 = rand_key("type", "key_");

        wallet.set(&my_key1, "value1").unwrap();
        let value = wallet.get(&my_key1).unwrap();
        assert_eq!("value1", value);

        wallet.set(&my_key1, "value2").unwrap();
        let value = wallet.get(&my_key1).unwrap();
        assert_eq!("value2", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_not_expired_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let config = RemoteWalletRuntimeConfig { 
            endpoint: String::from("http://localhost:8000/items/"), 
            freshness_time: 1
        };
        let cf_str = serde_json::to_string(&config).unwrap();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let my_key1 = rand_key("type", "key_");

        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
        wallet.set(&my_key1, "value1").unwrap();

        // Wait until value expires
        thread::sleep(Duration::new(5, 0));

        let value = wallet.get_not_expired(&my_key1);
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();
        let mut my_type = rand_type("type");
        let my_key = rand_key(&my_type, "key_");
        let mut my_key1 = my_key.clone();
        my_key1.push_str("1");
        let mut my_key2 = my_key.clone();
        my_key2.push_str("2");

        wallet.set(&my_key1, "value1").unwrap();
        wallet.set(&my_key2, "value2").unwrap();

        my_type.push_str("::");
        let mut key_values = wallet.list(&my_type).unwrap();
        key_values.sort();
        assert_eq!(2, key_values.len());

        let (key, value) = key_values.pop().unwrap();
        assert_eq!(my_key2, key);
        assert_eq!("value2", value);

        let (key, value) = key_values.pop().unwrap();
        assert_eq!(my_key1, key);
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let wallet_type = RemoteWalletType::new();
        wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let mut my_type = rand_type("type");
        let my_key = rand_key(&my_type, "key_");
        let mut my_key1 = my_key.clone();
        my_key1.push_str("1");
        let mut my_key2 = my_key.clone();
        my_key2.push_str("2");
        let mut my_key3 = my_key.clone();
        my_key3.push_str("3");
        let mut my_key4 = my_key.clone();
        my_key4.push_str("4");
        let mut my_key5 = my_key.clone();
        my_key5.push_str("5");

        let credentials1 = default_virtual_credentials_for_test(&token, "client1");
        let credentials2 = default_virtual_credentials_for_test(&token, "client2");

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials1)).unwrap();

            wallet.set(&my_key1, "value1").unwrap();
            wallet.set(&my_key2, "value2").unwrap();
        }

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials2)).unwrap();

            wallet.set(&my_key3, "value3").unwrap();
            wallet.set(&my_key4, "value4").unwrap();
            wallet.set(&my_key5, "value5").unwrap();
        }

        my_type.push_str("::");

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials1)).unwrap();

            let mut key_values = wallet.list(&my_type).unwrap();
            key_values.sort();
            assert_eq!(2, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!(my_key2, key);
            assert_eq!("value2", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!(my_key1, key);
            assert_eq!("value1", value);
        }

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials2)).unwrap();

            let mut key_values = wallet.list(&my_type).unwrap();
            key_values.sort();
            assert_eq!(3, key_values.len());

            let (key, value) = key_values.pop().unwrap();
            assert_eq!(my_key5, key);
            assert_eq!("value5", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!(my_key4, key);
            assert_eq!("value4", value);

            let (key, value) = key_values.pop().unwrap();
            assert_eq!(my_key3, key);
            assert_eq!("value3", value);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_get_pool_name_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let remote_wallet_type = RemoteWalletType::new();
        remote_wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = remote_wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();

        assert_eq!(wallet.get_pool_name(), "pool1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_get_name_works() {
        TestUtils::cleanup_indy_home();

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        // build credentials before creating and opening wallet
        let ac_str = default_credentials_for_test(&token);

        let remote_wallet_type = RemoteWalletType::new();
        remote_wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();
        let wallet = remote_wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&ac_str)).unwrap();

        assert_eq!(wallet.get_name(), "wallet1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn remote_wallet_credentials_deserialize() {
        let empty: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{}"#);
        // not an error for remote wallet
        //assert!(empty.is_err());
        assert!(empty.is_ok());

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

        let three: Result<RemoteWalletCredentials, JsonError> = serde_json::from_str(r#"{"auth_token":"","virtual_wallet":"thisismynewpassword"}"#);
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
    fn validate_add_ten_thousand_claims_and_see_what_happens() {
        TestUtils::cleanup_indy_home();

        println!("Add 100 claims each for 100 wallets.");

        // set configuration, including endpoint
        let cf_str = default_config_for_test();

        // verify server is running and get a token
        let token = verify_rest_server();

        let ac_str = default_credentials_for_test(&token);

        let remote_wallet_type = RemoteWalletType::new();
        remote_wallet_type.create("wallet1", Some(&cf_str), Some(&ac_str)).unwrap();

        let my_type = rand_type("type");

        for i in 0..100 {
            // build credentials before creating and opening wallet
            let my_wallet = format!("wallet_{:04}", i);
            println!("Wallet = {}", my_wallet);
            let credentials = default_virtual_credentials_for_test(&token, &my_wallet);

            let wallet = remote_wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials)).unwrap();

            for j in 0..100 {
                let my_key = rand_key(&my_type, "key_");
                wallet.set(&my_key, "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}").unwrap();
            }
        }

        println!("Now run a query on each client and see how long they take.");
        for i in 0..100 {
            // build credentials before creating and opening wallet
            let my_wallet = format!("wallet_{:04}", i);
            let credentials = default_virtual_credentials_for_test(&token, &my_wallet);

            let wallet = remote_wallet_type.open("wallet1", "pool1", None, Some(&cf_str), Some(&credentials)).unwrap();

            let response = wallet.list(&my_type);
            match response {
                Ok(v) => (),
                Err(e) => assert!(false, format!("{:?}", e))
            }
        }

        println!("Done");
    }
}
