extern crate rusqlcipher;
extern crate time;
extern crate indy_crypto;

use super::{Wallet, WalletType};

use errors::common::CommonError;
use errors::wallet::WalletError;
use utils::environment::EnvironmentUtils;

use self::rusqlcipher::Connection;
use self::time::Timespec;

// use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::ops::Sub;

use self::indy_crypto::utils::json::JsonDecodable;

/*
 * Implementation of a virtual wallet store.
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
 *     {key: "", virtual_wallet: "subject1_wallet"}
 * 
 * If the virtual_wallet is not specified then the wallet name is used as the virtual wallet.
 * 
 * This code is cloned from "default.rs" and an additional database column added to the
 * wallet database to specify the virtual wallet.
 * 
 * Key names can be duplicated across virtual wallets.
 */


#[derive(Deserialize)]
struct VirtualWalletRuntimeConfig {
    freshness_time: i64
}

impl<'a> JsonDecodable<'a> for VirtualWalletRuntimeConfig {}

impl Default for VirtualWalletRuntimeConfig {
    fn default() -> Self {
        VirtualWalletRuntimeConfig { freshness_time: 1000 }
    }
}

#[derive(Deserialize, Debug)]
struct VirtualWalletCredentials {
    key: String,
    rekey: Option<String>,
    virtual_wallet: Option<String>   // virtual wallet name (optional)
}

impl<'a> JsonDecodable<'a> for VirtualWalletCredentials {}

impl Default for VirtualWalletCredentials {
    fn default() -> Self {
        VirtualWalletCredentials { key: String::new(), rekey: None, virtual_wallet: None }
    }
}

struct VirtualWalletRecord {
    key: String,
    value: String,
    time_created: Timespec
}

struct VirtualWallet {
    wallet_name: String,
    pool_name: String,
    config: VirtualWalletRuntimeConfig,
    credentials: VirtualWalletCredentials
}

impl VirtualWallet {
    fn new(name: &str,
           pool_name: &str,
           config: VirtualWalletRuntimeConfig,
           credentials: VirtualWalletCredentials) -> VirtualWallet {
        VirtualWallet {
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
fn virtual_wallet_name(wallet_name: &str, credentials: &VirtualWalletCredentials) -> String {
    match credentials.virtual_wallet {
        Some(ref s) => s.to_string(),
        None => wallet_name.to_string()
    }
}

// Helper method to fetch claims
// wallet_name is either the virtual wallet or the root walllet
fn call_get_internal(root_wallet_name: &str, wallet_name: &str,
                    credentials: &VirtualWalletCredentials, key: &str) -> Result<String, WalletError> {
    let db = _open_connection(root_wallet_name, credentials)?;
    let result = db.query_row(
            "SELECT key, value, time_created FROM wallet 
            WHERE virtual_wallet = ?1 AND key = ?2 LIMIT 1",
            &[&wallet_name.to_string(), &key.to_string()], |row| {
                VirtualWalletRecord {
                    key: row.get(0),
                    value: row.get(1),
                    time_created: row.get(2)
                }
            });
        match result {
            Ok(record) => Ok(record.value),
            Err(why) => Err(WalletError::NotFound(format!("{:?}", why)))
        }
} 

impl Wallet for VirtualWallet {
    fn set(&self, key: &str, value: &str) -> Result<(), WalletError> {
        if self.credentials.rekey.is_some() {
            return Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid wallet credentials json"))));
        }
        
        _open_connection(root_wallet_name(&self.wallet_name).as_str(), &self.credentials)?
            .execute(
                "INSERT OR REPLACE INTO wallet 
                (virtual_wallet, key, value, time_created) 
                VALUES 
                (?1, ?2, ?3, ?4)",
                &[&virtual_wallet_name(&self.wallet_name, &self.credentials).as_str(), &key.to_string(), &value.to_string(), &time::get_time()])?;
        Ok(())
    }

    // get will first check the selected wallet, and if the key is not found, 
    // will *also* check the root wallet
    // keys shared between all virtual wallets can be stored once in the root
    fn get(&self, key: &str) -> Result<String, WalletError> {
        if self.credentials.rekey.is_some() {
            return Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid wallet credentials json"))));
        }

        let result = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                        &virtual_wallet_name(&self.wallet_name, &self.credentials),
                                        &self.credentials, key);
        match result {
            Ok(record) => Ok(record),
            Err(why) => {
                let result2 = call_get_internal(&root_wallet_name(&self.wallet_name), 
                                                &root_wallet_name(&self.wallet_name),
                                                &self.credentials, key);
                match result2 {
                    Ok(record2) => Ok(record2),
                    Err(why2) => Err(WalletError::NotFound(format!("{:?}", why2)))
                }
            }
        }
    }

    // list will return records only from the selected wallet (root or virtual)
    fn list(&self, key_prefix: &str) -> Result<Vec<(String, String)>, WalletError> {
        if self.credentials.rekey.is_some() {
            return Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid wallet credentials json"))));
        }

        let connection = _open_connection(root_wallet_name(&self.wallet_name).as_str(), &self.credentials)?;
        let mut stmt = connection.prepare("SELECT key, value, time_created 
                FROM wallet WHERE virtual_wallet = ?1 AND key like ?2 order by key")?;
        let records = stmt.query_map(&[&virtual_wallet_name(&self.wallet_name, &self.credentials).as_str(), &format!("{}%", key_prefix)], |row| {
            VirtualWalletRecord {
                key: row.get(0),
                value: row.get(1),
                time_created: row.get(2)
            }
        })?;

        let mut key_values = Vec::new();

        for record in records {
            let key_value = record?;
            key_values.push((key_value.key, key_value.value));
        }

        Ok(key_values)
    }

    // TODO get_not_expired will first check the selected wallet, and if the key is not found, 
    // will *also* check the root wallet
    // keys shared between all virtual wallets can be stored once in the root
    fn get_not_expired(&self, key: &str) -> Result<String, WalletError> {
        if self.credentials.rekey.is_some() {
            return Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid wallet credentials json"))));
        }

        let record = _open_connection(root_wallet_name(&self.wallet_name).as_str(), &self.credentials)?
            .query_row(
                "SELECT key, value, time_created 
                FROM wallet WHERE virtual_wallet = ?1 AND key = ?2 LIMIT 1",
                &[&virtual_wallet_name(&self.wallet_name, &self.credentials).as_str(), &key.to_string()], |row| {
                    VirtualWalletRecord {
                        key: row.get(0),
                        value: row.get(1),
                        time_created: row.get(2)
                    }
                })?;

        if self.config.freshness_time != 0
            && time::get_time().sub(record.time_created).num_seconds() > self.config.freshness_time {
            return Err(WalletError::NotFound(key.to_string()));
        }

        return Ok(record.value);
    }

    fn close(&self) -> Result<(), WalletError> { Ok(()) }

    fn get_pool_name(&self) -> String {
        self.pool_name.clone()
    }

    fn get_name(&self) -> String {
        self.wallet_name.clone()
    }
}

pub struct VirtualWalletType {}

impl VirtualWalletType {
    pub fn new() -> VirtualWalletType {
        VirtualWalletType {}
    }
}

impl WalletType for VirtualWalletType {
    fn create(&self, name: &str, config: Option<&str>, credentials: Option<&str>) -> Result<(), WalletError> {
        trace!("VirtualWalletType.create >> {}, with config {:?} and credentials {:?}", name, config, credentials);
        let root_name = root_wallet_name(&name);
        let path = _db_path(&root_name);
        if path.exists() {
            trace!("VirtualWalletType.create << path exists");
            return Err(WalletError::AlreadyExists(root_name.to_string()));
        }

        let runtime_auth = match credentials {
            Some(auth) => VirtualWalletCredentials::from_json(auth)?,
            None => VirtualWalletCredentials::default()
        };

        if runtime_auth.rekey.is_some() {
            return Err(WalletError::CommonError(CommonError::InvalidStructure(format!("Invalid wallet credentials json"))));
        }

        // note the addition of an extra database column to specify the virtual wallet
        // this can also be the root (if no virtual is specified)
        _open_connection(&root_name, &runtime_auth).map_err(map_err_trace!())?
            .execute("CREATE TABLE wallet 
            (
                virtual_wallet TEXT NOT NULL,
                key TEXT NOT NULL, 
                value TEXT NOT NULL, 
                time_created TEXT NOT NULL,
                PRIMARY KEY (virtual_wallet, key)
            )", &[])
            .map_err(map_err_trace!())?;
        trace!("VirtualWalletType.create <<");
        Ok(())
    }

    fn delete(&self, name: &str, config: Option<&str>, credentials: Option<&str>) -> Result<(), WalletError> {
        trace!("VirtualWalletType.delete {}, with config {:?} and credentials {:?}", name, config, credentials);
        // FIXME: parse and implement credentials!!!
        let root_name = root_wallet_name(&name);
        Ok(fs::remove_file(_db_path(&root_name)).map_err(map_err_trace!())?)
    }

    fn open(&self, name: &str, pool_name: &str, config: Option<&str>, runtime_config: Option<&str>, credentials: Option<&str>) -> Result<Box<Wallet>, WalletError> {
        let runtime_config = match runtime_config {
            Some(config) => VirtualWalletRuntimeConfig::from_json(config)?,
            None => VirtualWalletRuntimeConfig::default()
        };

        let mut runtime_auth = match credentials {
            Some(auth) => VirtualWalletCredentials::from_json(auth)?,
            None => VirtualWalletCredentials::default()
        };

        let root_name = root_wallet_name(&name);
        _open_connection(&root_name, &runtime_auth).map_err(map_err_trace!())?
            .query_row("SELECT sql FROM sqlite_master", &[], |_| {})
            .map_err(map_err_trace!())?;

        if let Some(rekey) = runtime_auth.rekey {
            runtime_auth.key = rekey;
            runtime_auth.rekey = None;
        }

        Ok(Box::new(
            VirtualWallet::new(
                name,
                pool_name,
                runtime_config,
                runtime_auth)))
    }
}

fn _db_path(name: &str) -> PathBuf {
    let mut path = EnvironmentUtils::wallet_path(name);
    path.push("sqlite.db");
    path
}

fn _open_connection(name: &str, credentials: &VirtualWalletCredentials) -> Result<Connection, WalletError> {
    let path = _db_path(name);
    if !path.parent().unwrap().exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .create(path.parent().unwrap())?;
    }

    let conn = Connection::open(path)?;
    conn.execute(&format!("PRAGMA key='{}'", credentials.key), &[])?;

    match credentials.rekey {
        None => Ok(conn),
        Some(ref rk) => {
            if credentials.key.len() == 0 && rk.len() > 0 {
                _export_unencrypted_to_encrypted(conn, name, &rk)
            } else if rk.len() > 0 {
                conn.execute(&format!("PRAGMA rekey='{}'", rk), &[])?;
                Ok(conn)
            } else {
                _export_encrypted_to_unencrypted(conn, name)
            }
        }
    }
}

fn _export_encrypted_to_unencrypted(conn: Connection, name: &str) -> Result<Connection, WalletError> {
    let mut path = EnvironmentUtils::wallet_path(name);
    path.push("plaintext.db");

    conn.execute(&format!("ATTACH DATABASE {:?} AS plaintext KEY ''", path), &[])?;
    conn.query_row(&"SELECT sqlcipher_export('plaintext')", &[], |row| {})?;
    conn.execute(&"DETACH DATABASE plaintext", &[])?;
    let r = conn.close();
    if let Err((c, w)) = r {
        Err(WalletError::from(w))
    } else {
        let wallet = _db_path(name);
        fs::remove_file(&wallet)?;
        fs::rename(&path, &wallet)?;

        Ok(Connection::open(wallet)?)
    }
}

fn _export_unencrypted_to_encrypted(conn: Connection, name: &str, key: &str) -> Result<Connection, WalletError> {
    let mut path = EnvironmentUtils::wallet_path(name);
    path.push("encrypted.db");

    let sql = format!("ATTACH DATABASE {:?} AS encrypted KEY '{}'", path, key);
    conn.execute(&sql, &[])?;
    conn.query_row(&"SELECT sqlcipher_export('encrypted')", &[], |row| {})?;
    conn.execute(&"DETACH DATABASE encrypted", &[])?;
    let r = conn.close();
    if let Err((c, w)) = r {
        Err(WalletError::from(w))
    } else {
        let wallet = _db_path(name);
        fs::remove_file(&wallet)?;
        fs::rename(&path, &wallet)?;

        let new = Connection::open(wallet)?;
        new.execute(&format!("PRAGMA key='{}'", key), &[])?;
        Ok(new)
    }
}
/* TODO this code is duplicated from default.rs and causes a compile error
impl From<rusqlcipher::Error> for WalletError {
    fn from(err: rusqlcipher::Error) -> WalletError {
        match err {
            rusqlcipher::Error::QueryReturnedNoRows => WalletError::NotFound(format!("Wallet record is not found: {}", err.description())),
            rusqlcipher::Error::SqliteFailure(err, _) if err.code == rusqlcipher::ErrorCode::NotADatabase =>
                WalletError::AccessFailed(format!("Wallet security error: {}", err.description())),
            _ => WalletError::CommonError(CommonError::InvalidState(format!("Unexpected SQLite error: {}", err.description())))
        }
    }
}
*/

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
        
        let credentials1 = VirtualWalletCredentials{key: String::from("key"), rekey: None, 
                            virtual_wallet: Some(String::from("virtual"))};
        let w2 = virtual_wallet_name("root", &credentials1);
        assert_eq!("virtual", w2);
        
        let w3 = root_wallet_name("root");
        assert_eq!("root", w3);
        
        let credentials2 = VirtualWalletCredentials{key: String::from("key"), rekey: None, 
                            virtual_wallet: None};
        let w4 = virtual_wallet_name("root", &credentials2);
        assert_eq!("root", w4);
    }

    #[test]
    fn virtual_wallet_type_new_works() {
        VirtualWalletType::new();
    }

    #[test]
    fn virtual_wallet_type_create_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_type_create_works_for_twice() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let res = wallet_type.create("wallet1", None, None);
        assert_match!(Err(WalletError::AlreadyExists(_)), res);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_type_delete_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        wallet_type.delete("wallet1", None, None).unwrap();
        wallet_type.create("wallet1", None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_type_open_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_virtual_wallet_type_open_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"key":"","virtual_wallet":"some1"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();

        let credentials2 = Some(r#"{"key":"","virtual_wallet":"some2"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();

        let credentials3 = Some(r#"{"key":"","virtual_wallet":"some3"}"#);
        wallet_type.open("wallet1", "pool1", None, None, credentials3).unwrap();

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        wallet.set("key1", "value1").unwrap();
        let value = wallet.get("key1").unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_virtual_wallet_set_get_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"key":"","virtual_wallet":"some1"}"#);
        let credentials2 = Some(r#"{"key":"","virtual_wallet":"some2"}"#);
        let credentials3 = Some(r#"{"key":"","virtual_wallet":"some3"}"#);

        {
            let wallet1 = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();
            wallet1.set("key1", "value1").unwrap();
            let value1 = wallet1.get("key1").unwrap();
            assert_eq!("value1", value1);
        }

        {
            let wallet2 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            wallet2.set("key1", "value2").unwrap();
            let value2 = wallet2.get("key1").unwrap();
            assert_eq!("value2", value2);
        }

        {
            let wallet3 = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();
            let value3 = wallet3.get("key1").unwrap();
            assert_eq!("value1", value3);
        }

        {
            let wallet4 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet4.set("key1", "value_root").unwrap();
            let value4 = wallet4.get("key1").unwrap();
            assert_eq!("value_root", value4);
        }

        {
            let wallet5 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            let value5 = wallet5.get("key1").unwrap();
            assert_eq!("value2", value5);
        }

        {
            let wallet6 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            let value6 = wallet6.get("key1").unwrap();
            assert_eq!("value_root", value6);
        }

        // create key in root and fetch in virtual wallet
        {
            let wallet7 = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet7.set("root_only_key", "value_root_only").unwrap();
            let value7 = wallet7.get("root_only_key").unwrap();
            assert_eq!("value_root_only", value7);
        }
        {
            let wallet8 = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
            let value8 = wallet8.get("root_only_key").unwrap();
            assert_eq!("value_root_only", value8);
        }

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_reopen() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        {
            let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
            wallet.set("key1", "value1").unwrap();
        }

        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
        let value = wallet.get("key1").unwrap();
        assert_eq!("value1", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_get_works_for_unknown() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();
        let value = wallet.get("key1");
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_works_for_update() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        wallet.set("key1", "value1").unwrap();
        let value = wallet.get("key1").unwrap();
        assert_eq!("value1", value);

        wallet.set("key1", "value2").unwrap();
        let value = wallet.get("key1").unwrap();
        assert_eq!("value2", value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_set_get_not_expired_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();
        let wallet = wallet_type.open("wallet1", "pool1", None, Some("{\"freshness_time\": 1}"), None).unwrap();
        wallet.set("key1", "value1").unwrap();

        // Wait until value expires
        thread::sleep(Duration::new(2, 0));

        let value = wallet.get_not_expired("key1");
        assert_match!(Err(WalletError::NotFound(_)), value);

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
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
    fn virtual_virtual_wallet_list_works() {
        TestUtils::cleanup_indy_home();

        let wallet_type = VirtualWalletType::new();
        wallet_type.create("wallet1", None, None).unwrap();

        let credentials1 = Some(r#"{"key":"","virtual_wallet":"client1"}"#);
        let credentials2 = Some(r#"{"key":"","virtual_wallet":"client2"}"#);

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
    fn virtual_wallet_get_pool_name_works() {
        TestUtils::cleanup_indy_home();

        let virtual_wallet_type = VirtualWalletType::new();
        virtual_wallet_type.create("wallet1", None, None).unwrap();
        let wallet = virtual_wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        assert_eq!(wallet.get_pool_name(), "pool1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_get_name_works() {
        TestUtils::cleanup_indy_home();

        let virtual_wallet_type = VirtualWalletType::new();
        virtual_wallet_type.create("wallet1", None, None).unwrap();
        let wallet = virtual_wallet_type.open("wallet1", "pool1", None, None, None).unwrap();

        assert_eq!(wallet.get_name(), "wallet1");

        TestUtils::cleanup_indy_home();
    }

    #[test]
    fn virtual_wallet_credentials_deserialize() {
        let empty: Result<VirtualWalletCredentials, JsonError> = serde_json::from_str(r#"{}"#);
        assert!(empty.is_err());

        let one: Result<VirtualWalletCredentials, JsonError> = serde_json::from_str(r#"{"key":""}"#);
        assert!(one.is_ok());
        let rone = one.unwrap();
        assert_eq!(rone.key, "");
        assert_eq!(rone.rekey, None);

        let two: Result<VirtualWalletCredentials, JsonError> = serde_json::from_str(r#"{"key":"thisisatest","rekey":null}"#);
        assert!(two.is_ok());
        let rtwo = two.unwrap();
        assert_eq!(rtwo.key, "thisisatest");
        assert_eq!(rtwo.rekey, None);

        let three: Result<VirtualWalletCredentials, JsonError> = serde_json::from_str(r#"{"key":"","rekey":"thisismynewpassword"}"#);
        assert!(three.is_ok());
        let rthree = three.unwrap();
        assert_eq!(rthree.key, "");
        assert_eq!(rthree.rekey, Some("thisismynewpassword".to_string()));

        let four: Result<VirtualWalletCredentials, JsonError> = serde_json::from_str(r#"{"key": "", "rekey": ""}"#);
        assert!(four.is_ok());
        let rfour = four.unwrap();
        assert_eq!(rfour.key, "");
        assert_eq!(rfour.rekey, Some("".to_string()));
    }

    #[test]
    fn virtual_wallet_convert_nonencrypted_to_encrypted() {
        TestUtils::cleanup_indy_home();
        {
            let virtual_wallet_type = VirtualWalletType::new();
            virtual_wallet_type.create("mywallet", None, Some(r#"{"key":""}"#)).unwrap();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":""}"#)).unwrap();

            wallet.set("key1::subkey1", "value1").unwrap();
            wallet.set("key1::subkey2", "value2").unwrap();
        }
        {
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":"", "rekey":"thisisatest"}"#)).unwrap();
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
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":"thisisatest"}"#)).unwrap();

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
    fn virtual_wallet_convert_encrypted_to_nonencrypted() {
        TestUtils::cleanup_indy_home();
        {
            let virtual_wallet_type = VirtualWalletType::new();
            virtual_wallet_type.create("mywallet", None, Some(r#"{"key":"thisisatest"}"#)).unwrap();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":"thisisatest"}"#)).unwrap();

            wallet.set("key1::subkey1", "value1").unwrap();
            wallet.set("key1::subkey2", "value2").unwrap();
        }
        {
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":"thisisatest", "rekey":""}"#)).unwrap();
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
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("mywallet", "pool1", None, None, Some(r#"{"key":""}"#)).unwrap();

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
/* TODO this is causing a compile error, fix later ...
    #[test]
    fn virtual_wallet_create_encrypted() {
        TestUtils::cleanup_indy_home();

        {
            let virtual_wallet_type = VirtualWalletType::new();
            virtual_wallet_type.create("encrypted_wallet", None, Some(r#"{"key":"test"}"#)).unwrap();
            let wallet = virtual_wallet_type.open("encrypted_wallet", "pool1", None, None, Some(r#"{"key":"test"}"#)).unwrap();

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
        }
        {
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet_error = virtual_wallet_type.open("encrypted_wallet", "pool1", None, None, None);

            match wallet_error {
                Ok(_) => assert!(false),
                Err(error) => assert_eq!(error.description(), String::from("Wallet security error: File opened that is not a database file"))
            };
        }

        TestUtils::cleanup_indy_home();
    }
*/
    #[test]
    fn virtual_wallet_change_key() {
        TestUtils::cleanup_indy_home();

        {
            let virtual_wallet_type = VirtualWalletType::new();
            virtual_wallet_type.create("encrypted_wallet", None, Some(r#"{"key":"test"}"#)).unwrap();
            let wallet = virtual_wallet_type.open("encrypted_wallet", "pool1", None, None, Some(r#"{"key":"test"}"#)).unwrap();

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
        }

        {
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("encrypted_wallet", "pool1", None, None, Some(r#"{"key":"test","rekey":"newtest"}"#)).unwrap();

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
            let virtual_wallet_type = VirtualWalletType::new();
            let wallet = virtual_wallet_type.open("encrypted_wallet", "pool1", None, None, Some(r#"{"key":"newtest"}"#)).unwrap();

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
