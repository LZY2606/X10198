use std::net::SocketAddr;

use phase_weaver::{db, seed, web};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut addr = "127.0.0.1:5258".to_string();
    let mut database = "phase_weaver.sqlite3".to_string();
    let mut seed_demo = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => addr = args.next().ok_or("missing --addr value")?,
            "--db" => database = args.next().ok_or("missing --db value")?,
            "--no-seed" => seed_demo = false,
            "--seed" => seed_demo = true,
            other if other.starts_with("--addr=") => addr = other["--addr=".len()..].to_string(),
            other if other.starts_with("--db=") => database = other["--db=".len()..].to_string(),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let mut database = db::Db::open(&database)?;
    let empty = database.tx(|conn| {
        db::ensure_branches(conn)?;
        let count: i64 = conn.query_row("select count(*) from imports", [], |r| r.get(0))?;
        Ok(count == 0)
    })?;
    if seed_demo && empty {
        database.tx(seed::seed_demo)?;
        println!("已导入离线演示家系批次（合成数据）");
    }

    let socket: SocketAddr = addr.parse()?;
    let listener = tokio::net::TcpListener::bind(socket).await?;
    println!("相位织图正在离线运行：http://{socket}");
    axum::serve(listener, web::app(database, 32, "main".into())).await?;
    Ok(())
}
