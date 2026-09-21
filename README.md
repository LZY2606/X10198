# 相位织图

离线家系基因型与单倍型分相审阅应用，使用 Rust、Axum 与 SQLite。原始导入按版本不可变保存；接受候选、局部锁定、撤回 read link、关系修订和回退都作为分支事件追加，重启后可逐步重放。

## 运行

```bash
cargo build --all-targets
cargo test --all-targets
cargo run -- --addr 127.0.0.1:5258
```

打开 http://127.0.0.1:5258 ，页面标题为“相位织图”。首次启动会在本地 SQLite 自动写入一份合成小家系演示数据；不会上传数据，也不会访问外部参考库。

## 主要接口

- `GET /api/state`：查看版本、谱系、变异矩阵、phase block、候选、冲突、忽略数据和裁定事件。
- `POST /api/import`：导入合成 JSON 批次，原始批次和观测按版本保留。
- `POST /api/decisions`：接受候选、锁定局部相位、撤回 read link、修订或标记关系。
- `POST /api/branches`：从当前分支创建裁定分支。
- `POST /api/pin`：钉住输入版本。
- `POST /api/rollback`：追加回退事件。
- `GET /api/export`：导出钉住输入版本、block、冲突和裁定事件。
- `GET /api/replay`：输出逐步重放记录。

候选搜索有预算上限；达到预算时 block 会明确标记，不会把截断结果伪装成唯一解。
