use std::borrow::{Borrow, BorrowMut};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::RwLock;
use std::task::{Context, Poll};

use actix_service::{Service, Transform};
use actix_web::{Error, HttpMessage, ResponseError};
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::{header::SET_COOKIE, HeaderValue};
use derive_more::{Display, From};
use futures_util::future::{FutureExt, LocalBoxFuture, ok, Ready};
use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use serde::__private::PhantomData;
use serde_json::error::Error as JsonError;

use crate::server_session_inner::ServerSessionInner;
use crate::server_session_state::{ServerSessionState, State};
use crate::session::{Session, SessionStatus};

lazy_static! {
    static ref STATE_SERVER: RwLock<ServerSessionState> = RwLock::new(ServerSessionState::new());
}

pub struct ServerSession(Rc<ServerSessionInner>);

impl ServerSession {
    pub fn new() -> ServerSession {
        STATE_SERVER.write().unwrap().start();
        ServerSession(Rc::new(ServerSessionInner::new()))
    }
}

impl<S, B: 'static> Transform<S> for ServerSession
    where
        S: Service<Request=ServiceRequest, Response=ServiceResponse<B>>,
        S::Future: 'static,
        S::Error: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Transform = ServerSessionMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ServerSessionMiddleware {
            service,
            inner: self.0.clone(),
        })
    }
}

pub struct ServerSessionMiddleware<S> {
    service: S,
    inner: Rc<ServerSessionInner>,
}

impl<S, B: 'static> Service for ServerSessionMiddleware<S>
    where
        S: Service<Request=ServiceRequest, Response=ServiceResponse<B>>,
        S::Future: 'static,
        S::Error: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    /// On first request, a new session cookie is returned in response, regardless
    /// of whether any session state is set.  With subsequent requests, if the
    /// session state changes, then set-cookie is returned in response.  As
    /// a user logs out, call session.purge() to set SessionStatus accordingly
    /// and this will trigger removal of the session cookie in the response.
    fn call(&mut self, mut req: ServiceRequest) -> Self::Future {
        let inner = self.inner.clone();
        let (mut is_new, mut id) = inner.get_session_id(&req);

        if let Some(state) = STATE_SERVER.read().unwrap().get_state(&id) {
            Session::set_session(state, &mut req);
        } else {
            is_new = true;
            id = inner.generate_id();
            Session::set_session(State::default(), &mut req);
        }

        let fut = self.service.call(req);

        let fut = async move {
            fut.await.map(|mut res| {
                if is_new {
                    inner.set_cookie(&mut res, id.clone());
                }
                match Session::get_changes(&mut res) {
                    (SessionStatus::Changed, Some(state))
                    | (SessionStatus::Renewed, Some(state)) => {
                        res.checked_expr(|res| {
                            STATE_SERVER.write().unwrap().set_state(&id, &state)
                        })
                    }
                    (SessionStatus::Unchanged, Some(state)) => {
                        res.checked_expr(|res| {
                            STATE_SERVER.write().unwrap().set_state(&id, &state)
                        })
                    }
                    (SessionStatus::Unchanged, _) => {
                        // set a new session cookie upon first request (new client)
                        res
                    }
                    (SessionStatus::Purged, _) => {
                        let _ = inner.remove_cookie(&mut res);
                        let _ = STATE_SERVER.write().unwrap().remove_state(&id);
                        res
                    }
                    _ => res,
                }
            })
        }.boxed_local();

        fut
    }
}