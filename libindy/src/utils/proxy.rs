extern crate hyper;
extern crate serde_json;
extern crate reqwest;
extern crate url;

use std::str;
use std::collections::HashMap;
use hyper::header::{Headers, Authorization};
use reqwest::{Response};
use serde_json::{Value};
use url::percent_encoding::{utf8_percent_encode, percent_decode, DEFAULT_ENCODE_SET};


#[derive(Debug)]
pub enum RestError {
    RestRequestError(String),
    RestServerError(String),
    RestOtherError(String),
    RestInvalidResponse(String),
    RestResponseFormatError(String)
}


pub fn ensure_trailing_slash(s: &str) -> String {
    let mut ss = s.to_owned();
    if !ss.ends_with("/") {
        ss.push_str("/");
    }
    ss
}

pub fn rest_append_path(endpoint: &str, app_path: &str) -> String {
    let mut ret_endpoint = ensure_trailing_slash(endpoint);
    let app_path_encoded = utf8_percent_encode(app_path, DEFAULT_ENCODE_SET).to_string();
    ret_endpoint.push_str(&app_path_encoded);
    ensure_trailing_slash(&ret_endpoint)
}

pub fn rest_endpoint(endpoint: &str, virtual_wallet_name: Option<&str>) -> String {
    let mut ret_endpoint = ensure_trailing_slash(endpoint);
    match virtual_wallet_name {
        Some(s) => {
            let s_encoded = utf8_percent_encode(s, DEFAULT_ENCODE_SET).to_string();
            ret_endpoint.push_str(&s_encoded)
        },
        _ => ()
    }
    ensure_trailing_slash(&ret_endpoint)
}

pub fn rest_resource_endpoint(endpoint: &str, resource_id: &str) -> String {
    rest_append_path(endpoint, resource_id)    
}

pub fn rest_auth_headers(auth_token: &str) -> Headers {
    let mut headers = Headers::new();
    let mut auth_str = "Token ".to_owned();
    auth_str.push_str(auth_token);
    headers.set(Authorization(auth_str));
    headers
}

pub fn rest_get_request(req_url: &str, headers: Option<Headers>) -> Result<Response, RestError> {
    let client = reqwest::Client::new();
    let mut rb = client.get(req_url);
    if headers != None {
        let _unused = rb.headers(headers.unwrap());
    }
    let res = rb.send();
    match res {
        Ok(r) => Ok(r),
        Err(e) => Err(RestError::RestRequestError(format!("{:?}", e)))
    }
}

pub fn rest_post_request(req_url: &str, headers: Option<Headers>, body: Option<&str>) -> Result<Response, RestError> {
    let client = reqwest::Client::new();
    let mut rb = client.post(req_url);
    if headers != None {
        let _unused = rb.headers(headers.unwrap());
    }
    if body != None {
        let mut j_headers = Headers::new();
        j_headers.set(reqwest::header::ContentType::json());
        let s = body.unwrap().to_owned();
        let _unused = rb.headers(j_headers).body(s);
    }
    let res = rb.send();
    match res {
        Ok(r) => Ok(r),
        Err(e) => Err(RestError::RestRequestError(format!("{:?}", e)))
    }
}

pub fn rest_post_request_map(req_url: &str, headers: Option<Headers>, body: Option<&HashMap<&str, &str>>) -> Result<Response, RestError> {
    let client = reqwest::Client::new();
    let mut rb = client.post(req_url);
    if headers != None {
        let _unused = rb.headers(headers.unwrap());
    }
    if body != None {
        let _unused = rb.json(body.unwrap());
    }
    let res = rb.send();
    match res {
        Ok(r) => Ok(r),
        Err(e) => Err(RestError::RestRequestError(format!("{:?}", e)))
    }
}

pub fn rest_put_request_map(req_url: &str, headers: Option<Headers>, body: Option<&HashMap<&str, &str>>) -> Result<Response, RestError> {
    let client = reqwest::Client::new();
    let mut rb = client.put(req_url);
    if headers != None {
        let _unused = rb.headers(headers.unwrap());
    }
    if body != None {
        let _unused = rb.json(body.unwrap());
    }
    let res = rb.send();
    match res {
        Ok(r) => Ok(r),
        Err(e) => Err(RestError::RestRequestError(format!("{:?}", e)))
    }
}

pub fn rest_post_request_auth(req_url: &str, userid: &str, password: &str) -> Result<String, RestError> {
    let mut map = HashMap::new();
    map.insert("username", userid);
    map.insert("password", password);
    let response = rest_post_request_map(req_url, None, Some(&map));
    match response {
        Ok(r) => rest_extract_response_item(r, "token"),
        Err(e) => Err(e)
    }
}

pub fn rest_check_result(mut res: Response) -> Result<(), RestError> {
    if res.status().is_success() {
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        match str::from_utf8(buf.as_slice()) {
            Ok(v) => v,
            Err(e) => return Err(RestError::RestInvalidResponse(format!("Invalid UTF-8 sequence: {:?}", e)))
        };
        Ok(())
    } else if res.status().is_server_error() {
        Err(RestError::RestServerError(format!("Server error {:?}", res.status())))
    } else {
        Err(RestError::RestOtherError(format!("Something else happened {:?}", res.status())))
    }
}

pub fn rest_extract_response_body(mut res: Response) -> Result<String, RestError> {
    if res.status().is_success() {
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        match str::from_utf8(buf.as_slice()) {
            Ok(v) => Ok(v.to_owned()),
            Err(e) => Err(RestError::RestInvalidResponse(format!("Invalid UTF-8 sequence: {:?}", e)))
        }
    } else if res.status().is_server_error() {
        Err(RestError::RestServerError(format!("Server error {:?}", res.status())))
    } else {
        Err(RestError::RestOtherError(format!("Something else happened {:?}", res.status())))
    }
}

pub fn rest_extract_response_item(mut res: Response, item_name: &str) -> Result<String, RestError> {
    if res.status().is_success() {
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        match str::from_utf8(buf.as_slice()) {
            Ok(v) => {
                // Parse the string of data into serde_json::Value.
                let v: Value = serde_json::from_str(v).unwrap();
                // Access parts of the data by indexing with square brackets.
                let ss = v[item_name].as_str().unwrap();
                return Ok(ss.to_owned())
            },
            Err(e) => return Err(RestError::RestInvalidResponse(format!("Invalid UTF-8 sequence: {:?}", e)))
        };
    } else if res.status().is_server_error() {
        Err(RestError::RestServerError(format!("Server error {:?}", res.status())))
    } else {
        Err(RestError::RestOtherError(format!("Something else happened {:?}", res.status())))
    }
}

fn object_as_hashmap(obj: &serde_json::Map<String, serde_json::Value>, keys: &Vec<&str>) -> HashMap<String, String> {
    let mut hm: HashMap<String, String> = HashMap::new();
    for i in 0..keys.len() {
        let ok = obj.get(keys[i]);
        match ok {
            Some(v) => {
                let sv = match v.to_owned() {
                    serde_json::Value::String(s) => s,
                    _ => serde_json::to_string(&v).unwrap()
                };
                hm.insert(keys[i].to_owned(), sv.to_owned());
                ()
            },
            None => ()
        }
    }

    hm
}

pub fn body_as_vec(body: &str, keys: &Vec<&str>) -> Result<Vec<HashMap<String, String>>, RestError> {
    let mut r_values: Vec<HashMap<String, String>> = Vec::new();
    let r = serde_json::from_str(body);
    match r {
        Ok(v) => {
            match v {
                serde_json::Value::String(s) => {
                    return Err(RestError::RestResponseFormatError(format!("Expecting an object or array but got a string {}", s)));
                },
                serde_json::Value::Object(o) => {
                    let om = object_as_hashmap(&o, keys);
                    r_values.push(om);
                    ()
                },
                serde_json::Value::Array(a) => {
                    for i in 0..a.len() {
                        if a[i].is_object() {
                            let om = object_as_hashmap(a[i].as_object().unwrap(), keys);
                            r_values.push(om);
                        } else {
                            return Err(RestError::RestResponseFormatError(format!("Expecting an array of objects {:?}", a[i])));
                        }
                    }
                },
                _ => {
                    return Err(RestError::RestResponseFormatError(format!("Expecting an array of objects {:?}", v)));
                } 
            }
        },
        Err(e) => {
            return Err(RestError::RestResponseFormatError(format!("Expecting a json object or array {:?}", e)));
        }
    }

    Ok(r_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json;
    use self::serde_json::Error as JsonError;
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

    #[test]
    fn ensure_url_encoding_works() {
        let input = "confident, productive systems programming";

        let encoded = utf8_percent_encode(input, DEFAULT_ENCODE_SET).to_string();
        assert_eq!(encoded, "confident,%20productive%20systems%20programming");

        let decoded = percent_decode(encoded.as_bytes()).decode_utf8().unwrap();
        assert_eq!(decoded, "confident, productive systems programming");
    }

    #[test]
    fn ensure_trailing_slash_works() {
        let s1 = "http://my.host.com/api".to_owned();
        let s2 = "http://my.host.com/api/".to_owned();
        let s3 = ensure_trailing_slash(&s1);
        assert_eq!(s2, s3);
        
        let s4 = ensure_trailing_slash(&s3);
        assert_eq!(s3, s4);
    }

    #[test]
    fn rest_append_path_works() {
        let s1 = "http://my.host.com/api".to_owned();
        let s2 = "my_wallet".to_owned();
        let s3 = "http://my.host.com/api/my_wallet/".to_owned();
        let s4 = rest_append_path(&s1, &s2);
        assert_eq!(s3, s4);
    }

    #[test]
    fn rest_endpoint_works() {
        let s1 = "http://my.host.com/api/".to_owned();
        let s2 = Some("my_wallet");
        let s3 = "http://my.host.com/api/my_wallet/".to_owned();
        let s4 = rest_endpoint(&s1, s2);
        assert_eq!(s3, s4);

        let s5 = rest_endpoint(&s1, None);
        assert_eq!(s1, s5);
    }

    #[test]
    fn rest_resource_endpoint_works() {
        let s1 = "http://my.host.com/api/my_wallet/".to_owned();
        let s2 = "some_id".to_owned();
        let s3 = "http://my.host.com/api/my_wallet/some_id/".to_owned();
        let s4 = rest_resource_endpoint(&s1, &s2);
        assert_eq!(s3, s4);
    }

    #[test]
    fn rest_auth_headers_works() {
        let my_token = "Some_Token".to_owned();
        let headers = rest_auth_headers(&my_token);
        // TODO figure out hot to test this
    }

    #[test]
    fn validate_unauth_rest_get_works() {
        let endpoint = "http://localhost:8000/api/v1/schema/";
        let response = rest_get_request(endpoint, None);
        match response {
            Ok(r) => {
                let result = rest_check_result(r);
                match result {
                    Ok(()) => (),
                    Err(e) => assert!(false, format!("{:?}", e))
                }
            }
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test]
    fn validate_rest_invalid_url_works() {
        let endpoint = "http://localhost:8765/api/v1/schema/";   // assume we're not listening on this port
        let response = rest_get_request(endpoint, None);
        match response {
            Ok(_r) => assert!(false),   // should fail
            Err(_e) => ()
        }

        let endpoint = "http://notalocalhost:8000/api/v1/schema/"; // not a valid server
        let response = rest_get_request(endpoint, None);
        match response {
            Ok(_r) => assert!(false),  // should fail
            Err(_e) => ()
        }
    }

    #[test]
    fn validate_rest_authenticate_works() {
        let endpoint = "http://localhost:8000/api/v1/api-token-auth/";
        let response = rest_post_request_auth(endpoint, "ian", "pass1234");
        match response {
            Ok(_s) => (), // ok, returned a token
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test]
    fn validate_rest_auth_get_works() {
        let get_endpoint = "http://localhost:8000/api/v1/keyval/my_wallet/type/";
        let response = rest_get_request(get_endpoint, None);
        match response {
            Ok(r) => {
                let result = rest_check_result(r);
                match result {
                    Ok(()) => assert!(false),  // should fail with no token
                    Err(_e) => ()
                }
            }
            Err(e) => assert!(false, format!("{:?}", e))
        }

        let auth_endpoint = "http://localhost:8000/api/v1/api-token-auth/";
        let response = rest_post_request_auth(auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => {     // ok, returned a token, try the "GET" again
                let token = s;
                let headers = rest_auth_headers(&token);
                let response = rest_get_request(get_endpoint, Some(headers));
                match response {
                    Ok(r) => {
                        let result = rest_check_result(r);
                        match result {
                            Ok(()) => (),
                            Err(e) => assert!(false, format!("{:?}", e))   // should pass now with a token
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                }
            },
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test]
    fn validate_rest_auth_post_works() {
        let get_endpoint = "http://localhost:8000/api/v1/keyval/";
        let mut rng = thread_rng();
        let num: i32 = rng.gen_range(0, 999999);
        let snum: String = format!("{:06}", num);
        let mut json = "{\"wallet_name\":\"my_wallet\", 
                    \"item_type\":\"claim\", 
                    \"item_id\":\"".to_owned();
        json.push_str(&snum);
        json.push_str("\", \"item_value\":\"{\\\"this\\\":\\\"is\\\", \\\"a\\\":\\\"claim\\\", \\\"from\\\":\\\"rust\\\"}\"}");
        let mut rng = thread_rng();
        let num: i32 = rng.gen_range(0, 999999);
        let snum: String = format!("{:06}", num);
        let mut map = HashMap::new();
        map.insert("wallet_name", "Rust_Wallet");
        map.insert("item_type", "rust_claim");
        map.insert("item_id", &snum);
        map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
        let response = rest_post_request(get_endpoint, None, Some(&json));
        match response {
            Ok(r) => {
                let result = rest_check_result(r);
                match result {
                    Ok(()) => assert!(false),  // should fail with no token
                    Err(_e) => ()
                }
            }
            Err(e) => assert!(false, format!("{:?}", e))
        }

        let auth_endpoint = "http://localhost:8000/api/v1/api-token-auth/";
        let response = rest_post_request_auth(auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => {     // ok, returned a token, try the "GET" again
                let token = s;

                // try with json string
                let headers = rest_auth_headers(&token);
                let response = rest_post_request(get_endpoint, Some(headers), Some(&json));
                match response {
                    Ok(r) => {
                        let result = rest_check_result(r);
                        match result {
                            Ok(()) => (),
                            Err(e) => assert!(false, format!("{:?}", e))   // should pass now with a token
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                };

                // try with map (serialize to json)
                let headers = rest_auth_headers(&token);
                let response = rest_post_request_map(get_endpoint, Some(headers), Some(&map));
                match response {
                    Ok(r) => {
                        let result = rest_check_result(r);
                        match result {
                            Ok(()) => (),
                            Err(e) => assert!(false, format!("{:?}", e))   // should pass now with a token
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                }
            },
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test] 
    fn validate_body_as_vec_works() {
        let mut rng = thread_rng();
        let num: i32 = rng.gen_range(0, 999999);
        let snum: String = format!("{:06}", num);
        let mut body = "[
            {\"url\":\"http://localhost:8000/api/v1/keyval/1/\",
            \"id\":1,
            \"created\":\"2018-02-27T17:05:09.577673Z\",
            \"wallet_name\":\"Rust_Wallet\",
            \"item_type\":\"rust_claim\",
            \"item_id\":\"888\",
            \"item_value\":\"{\\\"this\\\":\\\"is\\\", \\\"a\\\":\\\"claim\\\", \\\"from\\\":\\\"rust\\\"}\",
            \"creator\":\"ian\"},
            {\"url\":\"http://localhost:8000/api/v1/keyval/2/\",
            \"id\":2,
            \"created\":\"2018-02-27T17:17:17.635730Z\",
            \"wallet_name\":\"Rust_Wallet\",
            \"item_type\":\"rust_claim\",
            \"item_id\":\"".to_owned();
         body.push_str(&snum);
         body.push_str("\",
            \"item_value\":\"{\\\"this\\\":\\\"is\\\", \\\"a\\\":\\\"claim\\\", \\\"from\\\":\\\"rust\\\"}\",
            \"creator\":\"ian\"}
            ]");

        let mut keys: Vec<&str> = Vec::new();
        keys.push("wallet_name");
        keys.push("item_type");
        keys.push("item_id");
        keys.push("item_value");
        keys.push("id");
        keys.push("created");
        let objects = body_as_vec(&body, &keys);
        match objects {
            Ok(v) => {
                assert_eq!(v.len(), 2);
                let hm = &v[0];
                assert_eq!("Rust_Wallet", hm["wallet_name"]);
                assert_eq!("rust_claim", hm["item_type"]);
                assert_eq!("888", hm["item_id"]);
                assert_eq!("2018-02-27T17:05:09.577673Z", hm["created"]);
                assert_eq!("1", hm["id"]);
                assert_eq!("{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}", hm["item_value"]);
            },
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test]
    fn validate_rest_auth_get_list_works() {
        let auth_endpoint = "http://localhost:8000/api/v1/api-token-auth/";
        let response = rest_post_request_auth(auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => {     // ok, returned a token, try the "GET" again
                let token = s;

                // try with map (serialize to json)
                let mut rng = thread_rng();
                let num: i32 = rng.gen_range(0, 999999);
                let snum: String = format!("{:06}", num);
                let mut map = HashMap::new();
                map.insert("wallet_name", "Rust_Wallet");
                map.insert("item_type", "rust_claim");
                map.insert("item_id", &snum);
                map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
                let headers = rest_auth_headers(&token);
                let get_endpoint = "http://localhost:8000/api/v1/keyval/";
                let response = rest_post_request_map(get_endpoint, Some(headers), Some(&map));
                match response {
                    Ok(r) => {
                        let result = rest_check_result(r);
                        match result {
                            Ok(()) => (),
                            Err(e) => assert!(false, format!("{:?}", e))   // should pass now with a token
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                }

                // try with map (serialize to json)
                let num: i32 = rng.gen_range(0, 999999);
                let snum: String = format!("{:06}", num);
                let mut map = HashMap::new();
                map.insert("wallet_name", "Rust_Wallet");
                map.insert("item_type", "rust_claim");
                map.insert("item_id", &snum);
                map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
                let headers = rest_auth_headers(&token);
                let response = rest_post_request_map(get_endpoint, Some(headers), Some(&map));
                match response {
                    Ok(r) => {
                        let result = rest_check_result(r);
                        match result {
                            Ok(()) => (),
                            Err(e) => assert!(false, format!("{:?}", e))   // should pass now with a token
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                }

                // now do a get list into an array
                let get_endpoint = "http://localhost:8000/api/v1/keyval/Rust_Wallet/rust_claim/";
                let headers = rest_auth_headers(&token);
                let response = rest_get_request(get_endpoint, Some(headers));
                let body = rest_extract_response_body(response.unwrap());
                match body {
                    Ok(s) => {
                        let mut keys: Vec<&str> = Vec::new();
                        keys.push("wallet_name");
                        keys.push("item_type");
                        keys.push("item_id");
                        keys.push("item_value");
                        keys.push("id");
                        keys.push("created");
                        let objects = body_as_vec(&s, &keys);
                        match objects {
                            Ok(v) => {
                                for i in 0..v.len() {
                                    let hm = &v[i];
                                    println!("wallet_name {}", hm["wallet_name"]);
                                    println!("item_type {}", hm["item_type"]);
                                    println!("item_id {}", hm["item_id"]);
                                    println!("created {}", hm["created"]);
                                    println!("id {}", hm["id"]);
                                    println!("item_value {}", hm["item_value"]);
                                }
                            },
                            Err(e) => assert!(false, format!("{:?}", e))
                        }
                    },
                    Err(e) => assert!(false, format!("{:?}", e))
                }
            },
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }
}
