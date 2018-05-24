extern crate base64;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate lazy_static;

extern crate openssl;

extern crate hyper;
extern crate reqwest;
extern crate rand;

extern crate url;
extern crate backtrace;


// Note that to use macroses from util inside of other modules it must me loaded first!
#[macro_use]
mod utils;

pub mod api;
mod commands;
mod errors;
mod services;
mod domain;

#[cfg(test)]
mod tests {
    //use super::*;

    #[test]
    fn dummy() {
        assert!(true, "Dummy check!");
    }
}
