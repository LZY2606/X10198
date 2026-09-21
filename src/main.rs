mod domain;
mod engine;
mod repository;
mod seed;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut addr: SocketAddr = "127.0.0.1:5258".parse()?;
    let mut database = "phase-weaver.sqlite".to_string();
    let mut budget = engine::DEFAULT_BUDGET;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--addr" => {
                index += 1;
                addr = args
                    .get(index)
                    .ok_or("missing --addr value")?
                    .parse()?;
            }
            "--db" => {
                index += 1;
                database = args.get(index).ok_or("missing --db value")?.clone();
            }
            "--candidate-budget" => {
                index += 1;
                budget = args
                    .get(index)
                    .ok_or("missing --candidate-budget value")?
                    .parse()?;
            }
            "--help" | "-h" => {
                println!("相位织图 --addr 127.0.0.1:5258 [--db phase-weaver.sqlite] [--candidate-budget 64]");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        index += 1;
    }

    let repository = Arc::new(repository::Repository::open(&database)?);
    let state = web::AppState {
        repository,
        budget,
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("相位织图 offline server: http://{addr}");
    axum::serve(listener, web::router(state)).await?;
    Ok(())
}
