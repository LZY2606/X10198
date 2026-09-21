use phase_weave::demo;
use phase_weave::store;
use phase_weave::web;

use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let addr = match args.iter().position(|value| value == "--addr") {
        Some(index) => args.get(index + 1).ok_or("缺少 --addr 的值，例如 --addr 127.0.0.1:5258")?,
        None => "127.0.0.1:5258",
    };
    let database = match args.iter().position(|value| value == "--db") {
        Some(index) => args.get(index + 1).ok_or("缺少 --db 的值，例如 --db phase_weave.sqlite3")?,
        None => "phase_weave.sqlite3",
    };
    let store = Arc::new(store::Store::open(database)?);
    if store.branches()?.is_empty() {
        store.import(demo::demo_payload())?;
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("相位织图正在本地监听 http://{addr}");
    axum::serve(listener, web::router(store)).await?;
    Ok(())
}
