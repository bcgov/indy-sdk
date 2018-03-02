extern crate hyper;
extern crate serde_json;
extern crate reqwest;

use std::str;
use std::collections::HashMap;
use hyper::header::{Headers, Authorization};
use reqwest::{Response};
use serde_json::{Value};


#[derive(Debug)]
pub enum RestError {
    RestRequestError(String),
    RestServerError(String),
    RestOtherError(String),
    RestInvalidResponse(String)
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
    ret_endpoint.push_str(app_path);
    ensure_trailing_slash(&ret_endpoint)
}

pub fn rest_endpoint(endpoint: &str, virtual_wallet_name: Option<&str>) -> String {
    let mut ret_endpoint = ensure_trailing_slash(endpoint);
    match virtual_wallet_name {
        Some(s) => ret_endpoint.push_str(s),
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

pub fn rest_post_request_auth(req_url: &str, userid: &str, password: &str) -> Result<String, RestError> {
    let mut map = HashMap::new();
    map.insert("username", userid);
    map.insert("password", password);
    let response = rest_post_request_map(req_url, None, Some(&map));
    match response {
        Ok(r) => rest_extract_response(r, "token"),
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

pub fn rest_extract_response(mut res: Response, item_name: &str) -> Result<String, RestError> {
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
            }
            Err(e) => return Err(RestError::RestInvalidResponse(format!("Invalid UTF-8 sequence: {:?}", e)))
        };
    } else if res.status().is_server_error() {
        Err(RestError::RestServerError(format!("Server error {:?}", res.status())))
    } else {
        Err(RestError::RestOtherError(format!("Something else happened {:?}", res.status())))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use serde_json;
    use self::serde_json::Error as JsonError;

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
        let endpoint = "http://localhost:8000/schema/";
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
        let endpoint = "http://localhost:8765/schema/";   // assume we're not listening on this port
        let response = rest_get_request(endpoint, None);
        match response {
            Ok(_r) => assert!(false),   // should fail
            Err(_e) => ()
        }

        let endpoint = "http://notalocalhost:8000/schema/"; // not a valid server
        let response = rest_get_request(endpoint, None);
        match response {
            Ok(_r) => assert!(false),  // should fail
            Err(_e) => ()
        }
    }

    #[test]
    fn validate_rest_authenticate_works() {
        let endpoint = "http://localhost:8000/api-token-auth/";
        let response = rest_post_request_auth(endpoint, "ian", "pass1234");
        match response {
            Ok(_s) => (), // ok, returned a token
            Err(e) => assert!(false, format!("{:?}", e))
        }
    }

    #[test]
    fn validate_rest_auth_get_works() {
        let get_endpoint = "http://localhost:8000/items/";
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

        let auth_endpoint = "http://localhost:8000/api-token-auth/";
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
        let get_endpoint = "http://localhost:8000/items/";
        let json = "{
                    \"wallet_name\":\"my_wallet\", 
                    \"item_type\":\"claim\", 
                    \"item_id\":\"1234567890\", 
                    \"item_value\":\"{\\\"this\\\":\\\"is\\\", \\\"a\\\":\\\"claim\\\", \\\"from\\\":\\\"rust\\\"}\"}";
        let mut map = HashMap::new();
        map.insert("wallet_name", "Rust_Wallet");
        map.insert("item_type", "rust_claim");
        map.insert("item_id", "888");
        map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
        let response = rest_post_request(get_endpoint, None, Some(json));
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

        let auth_endpoint = "http://localhost:8000/api-token-auth/";
        let response = rest_post_request_auth(auth_endpoint, "ian", "pass1234");
        match response {
            Ok(s) => {     // ok, returned a token, try the "GET" again
                let token = s;

                // try with json string
                let headers = rest_auth_headers(&token);
                let response = rest_post_request(get_endpoint, Some(headers), Some(json));
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
}
