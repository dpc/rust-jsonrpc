// Rust JSON-RPC Library
// Written in 2015 by
//     Andrew Poelstra <apoelstra@wpsoftware.net>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

use std::sync::{Arc, Mutex};

use reqwest::{Client as ReqwestClient, Request as ReqwestRequest, Method, Url};
use reqwest::header::{Headers, Authorization, Basic};
use strason::Json;

use error::Error;
use {Request, Response};

/// A handle to a remote JSONRPC server
pub struct Client {
    url: Url,
    user: Option<String>,
    pass: Option<String>,
    client: ReqwestClient,
    nonce: Arc<Mutex<u64>>
}

impl Client {
    /// Creates a new client
    pub fn new<U, P>(url: &str, user: U, pass: P) -> Client
    where
        U: Into<Option<String>>,
        P: Into<Option<String>>,
    {
        let (user, pass) = (user.into(), pass.into());
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(pass.is_none() || user.is_some());

        Client {
            url: Url::parse(url).unwrap(),
            user: user,
            pass: pass,
            client: ReqwestClient::new(),
            nonce: Arc::new(Mutex::new(0))
        }
    }

    /// Sends a request to a client
    pub fn execute(&self, request: Request) -> Result<Response, Error> {
        let body = Json::from_serialize(&request)?.to_bytes();

        // Setup connection
        let mut reqwest_request = ReqwestRequest::new(Method::Get, self.url.clone());
        let mut headers = Headers::new();

        if let Some(ref user) = self.user {
            let scheme = Basic {
                username: user.clone(),
                password: self.pass.clone(),
            };

            headers.set(Authorization(scheme));
        }

        *(reqwest_request.headers_mut()) = headers;
        *(reqwest_request.body_mut()) = Some(body.into());

        // Send request
        let mut stream = self.client.execute(reqwest_request)?;

        // nb we ignore stream.status since we expect the body
        // to contain information about any error
        let response: Response = Json::from_reader(&mut stream)?.into_deserialize()?;
        match response.jsonrpc {
            Some(ref jsonrpc) if &*jsonrpc == "2.0" => {}
            _ => return Err(Error::VersionMismatch),
        }

        if response.id != request.id {
            return Err(Error::NonceMismatch);
        }

        Ok(response)
    }

    /// Builds a request
    pub fn build_request<N>(&self, name: N, params: Vec<Json>) -> Request
    where
        N: ToString,
    {
        let mut nonce = self.nonce.lock().unwrap();
        *nonce += 1;

        Request {
            method: name.to_string(),
            params: params,
            id: Json::from(*nonce),
            jsonrpc: Some(String::from("2.0"))
        }
    }

    /// Accessor for the last-used nonce
    pub fn last_nonce(&self) -> u64 {
        *self.nonce.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        let client = Client::new("localhost", None, None);
        assert_eq!(client.last_nonce(), 0);
        let req1 = client.build_request("test".to_owned(), vec![]);
        assert_eq!(client.last_nonce(), 1);
        let req2 = client.build_request("test".to_owned(), vec![]);
        assert_eq!(client.last_nonce(), 2);
        assert!(req1 != req2);
    }
}

