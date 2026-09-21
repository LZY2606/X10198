use std::net::SocketAddr;
use std::sync::Arc;

use phase_weave::{api, App};

#[tokio::main]
async fn main() {
    let mut addr: SocketAddr = "127.0.0.1:5258".parse().unwrap();
    let mut db = "phase-weave.db".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => {
                addr = args
                    .next()
                    .expect("缺少 --addr 参数")
                    .parse()
                    .expect("无法解析地址");
            }
            "--db" => {
                db = args.next().expect("缺少 --db 参数");
            }
            other => {
                eprintln!("未知参数: {}（支持 --addr 与 --db）", other);
                std::process::exit(2);
            }
        }
    }

    let app = Arc::new(App::open(&db).expect("无法打开本地数据库"));
    let router = api::router(app).into_make_service();
    let listener = tokio::net::TcpListener::bind(addr).await.expect("无法绑定地址");
    println!("相位织图已启动（仅本地，离线运行）：http://{}", addr);
    axum::serve(listener, router).await.expect("服务器错误");
}
