use actix_web::{Error, HttpMessage, ResponseError};
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::SET_COOKIE;
use actix_web::http::HeaderValue;
use derive_more::{Display, From};
use rand::Rng;
use serde_json::error::Error as JsonError;
use time::{Duration, OffsetDateTime};

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
    pub(crate) name: String,
    pub(crate) path: String,
    key: Key,
    pub(crate) secure: bool,
    pub(crate) http_only: bool,
    pub(crate) lazy: bool,
    pub(crate) domain: Option<String>,
    pub(crate) max_age: Option<Duration>,
    pub(crate) expires_in: Option<Duration>,
    pub(crate) same_site: Option<SameSite>,
}

impl ServerSessionInner {
    pub fn new(key: &[u8]) -> Self {
        ServerSessionInner {
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            key: Key::derive_from(key),
            lazy: false,
            secure: false,
            http_only: true,
            domain: None,
            max_age: None,
            expires_in: None,
            same_site: None,
        }
    }

    pub fn get_session_id(&self, req: &ServiceRequest) -> (bool, String) {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies.iter() {
                if cookie.name() == self.name {
                    if let val = cookie.value() {
                        let key = val.to_string();
                        return (false, key);
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

        let val = HeaderValue::from_str(&cookie.to_string())?;
        res.headers_mut().append(SET_COOKIE, val);

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