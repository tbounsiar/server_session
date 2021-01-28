use std::collections::HashMap;

use actix_web::{Error, HttpMessage, ResponseError};
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::SET_COOKIE;
use actix_web::http::HeaderValue;
use derive_more::{Display, From};
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use serde_json::error::Error as JsonError;
use time::{Duration, OffsetDateTime};

use crate::session::SessionStatus;

/// Errors that can occur during handling cookie session
#[derive(Debug, From, Display)]
pub enum CookieSessionError {
    /// Size of the serialized session is greater than 4000 bytes.
    #[display(fmt = "Size of the serialized session is greater than 4000 bytes.")]
    Overflow,
    /// Fail to serialize session.
    #[display(fmt = "Fail to serialize session")]
    Serialize(JsonError),
}

impl ResponseError for CookieSessionError {}

pub enum CookieSecurity {
    Signed,
    Private,
}

pub struct ServerSessionInner {
    name: String,
    path: String,
    security: CookieSecurity,
    key: Key,
    secure: bool,
    http_only: bool,
    lazy: bool,
    domain: Option<String>,
    max_age: Option<Duration>,
    expires_in: Option<Duration>,
    same_site: Option<SameSite>,
    status: SessionStatus,
}

impl ServerSessionInner {

    pub fn new() -> Self {
        ServerSessionInner {
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            security: CookieSecurity::Signed,
            key: Key::derive_from(&[0; 32]),
            lazy: false,
            secure: false,
            http_only: true,
            domain: None,
            max_age: None,
            expires_in: None,
            same_site: None,
            status: SessionStatus::Unchanged,
        }
    }

    fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
    }

    pub fn get_session_id(&self, req: &ServiceRequest) -> (bool, String) {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies.iter() {
                if cookie.name() == self.name {
                    let mut jar = CookieJar::new();
                    jar.add_original(cookie.clone());

                    let cookie_opt = match self.security {
                        CookieSecurity::Signed => jar.signed(&self.key).get(&self.name),
                        CookieSecurity::Private => {
                            jar.private(&self.key).get(&self.name)
                        }
                    };
                    if let Some(cookie) = cookie_opt {
                        if let val = cookie.value() {
                            let key = val.to_string();
                            return (false, key);
                        }
                    }
                }
            }
        }
        let id = self.generate_id();
        (true, id)
    }

    pub fn generate_id(&self) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();
        let id: String = (0..32)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();
        id
    }

    pub fn set_cookie<B>(&self, res: &mut ServiceResponse<B>, value: String) -> Result<(), Error> {
        if self.lazy && value.is_empty() {
            return Ok(());
        }
        if value.len() > 4064 {
            return Err(CookieSessionError::Overflow.into());
        }

        let mut cookie = Cookie::new(self.name.clone(), value);
        cookie.set_path(self.path.clone());
        cookie.set_secure(self.secure);
        cookie.set_http_only(self.http_only);

        if let Some(ref domain) = self.domain {
            cookie.set_domain(domain.clone());
        }

        if let Some(expires_in) = self.expires_in {
            cookie.set_expires(OffsetDateTime::now_utc() + expires_in);
        }

        if let Some(max_age) = self.max_age {
            cookie.set_max_age(max_age);
        }

        if let Some(same_site) = self.same_site {
            cookie.set_same_site(same_site);
        }

        let mut jar = CookieJar::new();

        match self.security {
            CookieSecurity::Signed => jar.signed(&self.key).add(cookie),
            CookieSecurity::Private => jar.private(&self.key).add(cookie),
        }

        for cookie in jar.delta() {
            let val = HeaderValue::from_str(&cookie.encoded().to_string())?;
            res.headers_mut().append(SET_COOKIE, val);
        }

        Ok(())
    }

    /// invalidates session cookie
    pub fn remove_cookie<B>(&self, res: &mut ServiceResponse<B>) -> Result<(), Error> {
        let mut cookie = Cookie::named(self.name.clone());
        cookie.set_path(self.path.clone());
        cookie.set_value("");
        cookie.set_max_age(Duration::zero());
        cookie.set_expires(OffsetDateTime::now_utc() - Duration::days(365));

        let val = HeaderValue::from_str(&cookie.to_string())?;
        res.headers_mut().append(SET_COOKIE, val);

        Ok(())
    }
}