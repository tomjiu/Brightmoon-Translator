# ECDICT 对照（Moon vs pot 插件）

| | pot-app-translate-plugin-ecdict | Moon |
|--|--------------------------------|------|
| 数据 | `POST https://pot-app.com/api/dict`（**远程**） | 本地 SQLite `ecdict.db` + 可选 cloudflare 分片 |
| 用途 | 当作「翻译服务」返回词典结构 | 词典页 / 联想 / vocabulary 查询，**不是** MT 引擎 |
| 离线 | 名「离线」但依赖 pot 服务器 | **真离线**（库加载成功时） |

## 结论

- **不重做**第二份 ECDICT 引擎。  
- 若 `ecdict_pool` 未连接，词典命令会报「本地词典数据库未加载」——属诚实失败。  
- 学习/查词入口：DictionarySearch、Vocabulary 相关页已聚合 ECDICT。  

## 相关代码

- `src-tauri/src/commands/dictionary_cmd.rs`  
- `src-tauri/src/skills/dictionary.rs`  
- `cloudflare-api` 分片备份（可选在线）  
