use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::bail;

fn bind_addr() -> Result<SocketAddr, std::net::AddrParseError> {
    env::var("PGPAD_WEB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()
}

fn db_path() -> PathBuf {
    env::var_os("PGPAD_WEB_DB")
        .map(PathBuf::from)
        .unwrap_or_else(pgpad_web::default_db_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if !pgpad_web::has_index_html() {
        bail!("Could not find dist/index.html, run `npm run build` first")
    }

    let addr = bind_addr()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let state = pgpad_web::WebState::new(db_path())?;

    println!("Serving pgpad web assets");
    println!("Listening on http://{addr}/?token={}", state.auth_token());

    axum::serve(listener, pgpad_web::router(state)).await?;

    Ok(())
}
