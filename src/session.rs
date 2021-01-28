use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use actix_web::{Error, FromRequest, HttpMessage, HttpRequest};
use actix_web::dev::{Extensions, Payload, RequestHead, ServiceRequest, ServiceResponse};
use futures_util::future::{ok, Ready};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::server_session_state::State;

pub trait UserSession {
    fn get_session(&self) -> Session;
}

impl UserSession for HttpRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl UserSession for ServiceRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl UserSession for RequestHead {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum SessionStatus {
    Changed,
    Purged,
    Renewed,
    Unchanged,
}

impl Default for SessionStatus {
    fn default() -> SessionStatus {
        SessionStatus::Unchanged
    }
}

struct SessionInner {
    state: State,
    pub status: SessionStatus,
}

impl SessionInner {
    pub fn new(timeout: Duration) -> SessionInner {
        SessionInner {
            state: State::new(timeout),
            status: SessionStatus::default(),
        }
    }
}

pub struct Session(Rc<RefCell<SessionInner>>);

impl Session {
    /// Get a `value` from the session.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        self.0.borrow().state.get(key)
    }

    /// Set a `value` from the session.
    pub fn set<T: Serialize>(&self, key: &str, value: T) -> Result<(), Error> {
        let mut inner = self.0.borrow_mut();
        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            inner.state.set(key, &value);
        }
        Ok(())
    }

    pub fn update_timeout(&self, minutes: u64) {
        let mut inner = self.0.borrow_mut();
        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            inner.state.update_timeout(Duration::from_secs(minutes * 60));
        }
    }

    /// Remove value from the session.
    pub fn remove(&self, key: &str) {
        let mut inner = self.0.borrow_mut();
        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            inner.state.remove(key);
        }
    }

    /// Clear the session.
    pub fn clear(&self) {
        let mut inner = self.0.borrow_mut();
        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            inner.state.clear()
        }
    }

    /// Removes session, both client and server side.
    pub fn purge(&self) {
        let mut inner = self.0.borrow_mut();
        inner.status = SessionStatus::Purged;
        inner.state.clear();
    }

    /// Renews the session key, assigning existing session state to new key.
    pub fn renew(&self) {
        let mut inner = self.0.borrow_mut();
        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Renewed;
        }
    }

    /// Adds the given key-value pairs to the session on the request.
    ///
    /// Values that match keys already existing on the session will be overwritten. Values should
    /// already be JSON serialized.
    ///
    /// # Example
    ///
    /// ```
    /// # use actix_session::Session;
    /// # use actix_web::test;
    /// #
    /// let mut req = test::TestRequest::default().to_srv_request();
    ///
    /// Session::set_session(
    ///     vec![("counter".to_string(), serde_json::to_string(&0).unwrap())],
    ///     &mut req,
    /// );
    /// ```
    pub fn set_session(
        data: State,
        req: &mut ServiceRequest,
    ) {
        let session = Session::get_session(&mut *req.extensions_mut());
        let mut inner = session.0.borrow_mut();
        inner.state.update_timeout(data.timeout());
        inner.state.extend(data);
    }

    pub fn get_changes<B>(
        res: &mut ServiceResponse<B>,
    ) -> (
        SessionStatus,
        Option<State>,
    ) {
        if let Some(s_impl) = res
            .request()
            .extensions()
            .get::<Rc<RefCell<SessionInner>>>()
        {
            let timeout = s_impl.borrow().state.timeout().clone();
            let state =
                std::mem::replace(&mut s_impl.borrow_mut().state, State::new(timeout));
            (s_impl.borrow().status.clone(), Some(state))
        } else {
            (SessionStatus::Unchanged, None)
        }
    }

    fn get_session(extensions: &mut Extensions) -> Session {
        if let Some(s_impl) = extensions.get::<Rc<RefCell<SessionInner>>>() {
            return Session(Rc::clone(&s_impl));
        }
        let inner = Rc::new(RefCell::new(SessionInner::new(Duration::from_secs(10))));
        extensions.insert(inner.clone());
        Session(inner)
    }
}

impl FromRequest for Session {
    type Error = Error;
    type Future = Ready<Result<Session, Error>>;
    type Config = ();

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ok(Session::get_session(&mut *req.extensions_mut()))
    }
}