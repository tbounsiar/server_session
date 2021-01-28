#![deny(rust_2018_idioms)]
// #[macro_use]
// extern crate lazy_static;

use actix_web::{App, get, HttpServer, Responder, Result, web};

use crate::server_session::ServerSession;
use crate::session::Session;

mod server_session;
mod session;
mod server_session_inner;
mod server_session_state;

#[get("/")]
async fn index(session: Session) -> Result<&'static str> {
    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        session.set("counter", count + 1)?;
    } else {
        session.set("counter", 1)?;
    }
    Ok("Welcome!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .wrap(ServerSession::new())
            .service(index)
    })
        // .workers(1)
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
