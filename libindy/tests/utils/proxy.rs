extern crate hyper;
extern crate serde_json;
extern crate reqwest;
extern crate url;

use std::str;
use std::collections::HashMap;
use self::hyper::header::{Headers};
use self::reqwest::{Response};
use serde_json::{Value};


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
        Ok(r) => rest_extract_response_item(r, "token"),
        Err(e) => Err(e)
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
            Err(e) => {
                return Err(RestError::RestInvalidResponse(format!("Invalid UTF-8 sequence: {:?}", e)))
            }
        };
    } else if res.status().is_server_error() {
        Err(RestError::RestServerError(format!("Server error {:?}", res.status())))
    } else {
        Err(RestError::RestOtherError(format!("Something else happened {:?}", res.status())))
    }
}
