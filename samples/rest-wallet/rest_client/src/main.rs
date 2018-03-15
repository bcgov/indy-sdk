extern crate hyper;
extern crate serde_json;
extern crate reqwest;
extern crate rand;
extern crate time;

use std::str;
use std::collections::HashMap;
use hyper::header::{Headers, Authorization /*, Basic} */};
use serde_json::{Value /*, Error */};
use rand::{thread_rng, Rng};
use std::ops::Sub;

fn main() {
    println!("Hello, world!");

    let mut rng = thread_rng();
    let num: i32 = rng.gen_range(0, 999999);
    let snum: String = format!("{:06}", num);
    println!("Rand {}", snum);

    let timestr = "2018-03-04T19:37:36.612196Z";
    let timefmt = "%Y-%m-%dT%H:%M:%S";
    let time_created = time::strptime(timestr, timefmt).unwrap();
    let num_secs = time::now_utc().sub(time_created).num_seconds();

    let client = reqwest::Client::new();

    println!("Try to do an un-authenticated GET schema (should work)");
    let res1 = client.get("http://localhost:8000/schema/")
        .send()
        .unwrap();
    check_result(res1);

    println!("Try to do an un-authenticated GET list of items");
    let res1 = client.get("http://localhost:8000/items/")
        .send()
        .unwrap();
    check_result(res1);

    println!("Try to POST a JSON body un-authenticated");
    let mut map = HashMap::new();
    map.insert("wallet_name", "Rust_Wallet");
    map.insert("item_type", "rust_claim");
    map.insert("item_id", "666");
    map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
    let res2 = client.post("http://localhost:8000/items/")
        .json(&map)
        .send()
        .unwrap();
    check_result(res2);
/*
    println!("Try to POST a JSON body using basic auth");
    let mut map = HashMap::new();
    map.insert("wallet_name", "Rust_Wallet");
    map.insert("item_type", "rust_claim");
    map.insert("item_id", "777");
    map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
    let mut headers3 = Headers::new();
    headers3.set(Authorization(Basic {
           username: "wall-e".to_owned(),
           password: Some("pass1234".to_owned())
       }));
    let res3 = client.post("http://localhost:8000/items/")
        .headers(headers3)
        .json(&map)
        .send()
        .unwrap();
    check_result(res3);
*/
    println!("Try to login to get a DRF token");
    let mut map = HashMap::new();
    map.insert("username", "wall-e");
    map.insert("password", "pass1234");
    let res3 = client.post("http://localhost:8000/api-token-auth/")
        .json(&map)
        .send()
        .unwrap();
    let drf_token = check_result_token(res3);

    println!("Try to POST a JSON body using a DRF token");
    let json = "{\"wallet_name\":\"my_wallet\", 
                 \"item_type\":\"claim\", 
                 \"item_id\":\"1234567890\", 
                 \"item_value\":\"{\\\"this\\\":\\\"is\\\", \\\"a\\\":\\\"claim\\\", \\\"from\\\":\\\"rust\\\"}\"
                }";
    println!("{}", json);
    let mut map = HashMap::new();
    map.insert("wallet_name", "Rust_Wallet");
    map.insert("item_type", "rust_claim");
    map.insert("item_id", "888");
    map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
    let mut auth_str = "Token ".to_owned();
    let mut headers4 = Headers::new();
    auth_str.push_str(&drf_token);
    headers4.set(Authorization(auth_str));
    let mut headers4a = Headers::new();
    headers4a.set(reqwest::header::ContentType::json());
    let res4 = client.post("http://localhost:8000/items/")
        .headers(headers4)
        //.json(&map)
        .headers(headers4a)
        .body(json)
        .send()
        .unwrap();
    check_result(res4);

    println!("Now try to fetch items using the DRF token");
    let mut headers5 = Headers::new();
    let mut auth_str = "Token ".to_owned();
    auth_str.push_str(&drf_token);
    headers5.set(Authorization(auth_str));
    let res8 = client.get("http://localhost:8000/items/")
        .headers(headers5)
        .send()
        .unwrap();
    //check_result(res8);
    let ss = rest_extract_response_body(res8);

/*
    println!("Try to register a new user using a JWT token");
    let mut map = HashMap::new();
    map.insert("username", "wall-e");
    map.insert("password1", "pass1234");
    map.insert("password2", "pass1234");
    let res5 = client.post("http://localhost:8000/rest-auth/registration/")
        .json(&map)
        .send()
        .unwrap();
    check_result(res5);

    println!("Try to login using a JWT token");
    let mut map = HashMap::new();
    map.insert("username", "wall-e");
    map.insert("password", "pass1234");
    let res6 = client.post("http://localhost:8000/rest-auth/login/")
        .json(&map)
        .send()
        .unwrap();
    let jwt_token = check_result_token(res6);

    println!("Try to POST a JSON body using a JWT token");
    let mut map = HashMap::new();
    map.insert("wallet_name", "Rust_Wallet");
    map.insert("item_type", "rust_claim");
    map.insert("item_id", "999");
    map.insert("item_value", "{\"this\":\"is\", \"a\":\"claim\", \"from\":\"rust\"}");
    let mut headers7 = Headers::new();
    let mut auth_str = "JWT ".to_owned();
    auth_str.push_str(&jwt_token);
    headers7.set(Authorization(auth_str));
    let res7 = client.post("http://localhost:8000/items/")
        .headers(headers7)
        .json(&map)
        .send()
        .unwrap();
    check_result(res7);

    println!("Try to GET a JSON body using a JWT token");
    let mut headers8 = Headers::new();
    let mut auth_str = "JWT ".to_owned();
    auth_str.push_str(&jwt_token);
    headers8.set(Authorization(auth_str));
    let res8 = client.get("http://localhost:8000/items/")
        .headers(headers8)
        .send()
        .unwrap();
    check_result(res8);
*/
    //unserialize_into_json("[{\"s\":\"1\"}, {\"t\":\"2\"}]");
    unserialize_into_json(&ss);
}

fn check_result(mut res: reqwest::Response) {
    if res.status().is_success() {
        println!("success!");
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        let s = match str::from_utf8(buf.as_slice()) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };
        println!("{}", s);
    } else if res.status().is_server_error() {
        println!("server error!");
    } else {
        println!("Something else happened. Status: {:?}", res.status());
    }
}

fn check_result_token(mut res: reqwest::Response) -> String {
    if res.status().is_success() {
        println!("success!");
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        let s = match str::from_utf8(buf.as_slice()) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };
        println!("{}", s);

        // Parse the string of data into serde_json::Value.
        let v: Value = serde_json::from_str(s).unwrap();

        // Access parts of the data by indexing with square brackets.
        let ss = v["token"].as_str().unwrap();
        return ss.to_owned()
    } else if res.status().is_server_error() {
        println!("server error!");
    } else {
        println!("Something else happened. Status: {:?}", res.status());
    }
    "".to_owned()
}

pub fn rest_extract_response_body(mut res: reqwest::Response) -> String {
    if res.status().is_success() {
        let mut buf: Vec<u8> = vec![];
        let _cres = res.copy_to(&mut buf);
        let s = match str::from_utf8(buf.as_slice()) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e)
        };
        return s.to_owned();
    } else if res.status().is_server_error() {
        println!("server error!");
    } else {
        println!("Something else happened. Status: {:?}", res.status());
    }
    "".to_owned()
}

fn unserialize_into_json(s: &str) {
    println!("{}", s);
    let r = serde_json::from_str(s);
    match r {
        Ok(v) => {
            match v {
                serde_json::Value::String(s) => {
                    println!("Got a string {:?}", s)
                },
                serde_json::Value::Array(a) => {
                    println!("Got an array {:?}", a);
                    println!("Array len {}", a.len());
                    for i in 0..a.len() {
                        if a[i].is_object() {
                            println!("a {} is an object", i);
                            println!("wallet_name {}", a[i].as_object().unwrap()["wallet_name"]);
                            println!("item_type {}", a[i].as_object().unwrap()["item_type"]);
                            println!("item_id {}", a[i].as_object().unwrap()["item_id"]);
                            println!("created {}", a[i].as_object().unwrap()["created"]);
                            println!("id {}", a[i].as_object().unwrap()["id"]);
                            let rs = serde_json::to_string(&a[i].as_object().unwrap()["item_value"]).unwrap();
                            println!("item_value {}", rs);
                        } else {
                            println!("a {} is not an object", i);
                        }
                    }
                },
                serde_json::Value::Object(o) => {
                    println!("Got an object {:?}", o)
                },
                _ => {
                    println!("Got something else")
                } 
            }
        },
        Err(e) => println!("Error {:?}", e)
    }
}
