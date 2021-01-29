use actix_web::{App, get, HttpServer, Result};

use actix_server_session::{ServerSession, Session};

#[get("/")]
async fn index(session: Session) -> Result<&'static str> {

    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        session.set("counter", count + 1)?;
    } else {
        session.set("counter", 1)?;
        session.update_timeout(5);
    }
    Ok("Welcome!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .wrap(
                ServerSession::signed(&[0; 32])
                    .secure(false)
                    .set_timeout(1)
            )
            .service(index)
    })
        // .workers(1)
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
