use phase_weaver::db::Db;
use phase_weaver::{db, web};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn index_page_shows_phase_weaver_title() {
    let mut database = Db::open(":memory:").unwrap();
    database.tx(db::ensure_branches).unwrap();
    let app = web::app(database, 32, "main".into());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut body = String::new();
    stream.read_to_string(&mut body).await.unwrap();
    assert!(body.contains("<h1>相位织图</h1>"));
    assert!(body.contains("不访问外部参考库"));
}
