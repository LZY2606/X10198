use phase_weave::server;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut addr = "127.0.0.1:5258".to_string();
    let mut db = "phase-weave.db".to_string();
    let args: Vec<String> = std::env::args().collect();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--addr" if index + 1 < args.len() => {
                addr = args[index + 1].clone();
                index += 2;
            }
            "--db" if index + 1 < args.len() => {
                db = args[index + 1].clone();
                index += 2;
            }
            _ => {
                eprintln!("用法: phase-weave [--addr 127.0.0.1:5258] [--db phase-weave.db]");
                std::process::exit(2);
            }
        }
    }
    let store = server::seeded_store(&db)?;
    let app = server::router(store);
    let socket: SocketAddr = addr.parse()?;
    let listener = tokio::net::TcpListener::bind(socket).await?;
    println!("相位织图: http://{}", socket);
    axum::serve(listener, app).await?;
    Ok(())
}
