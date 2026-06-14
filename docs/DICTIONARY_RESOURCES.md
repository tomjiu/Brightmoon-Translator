# 词典资源整理

生成时间: 2026-06-14
状态: **推荐方案已确定**

---

## 🎯 推荐组合方案

针对 Rust + Tauri 语言学习应用，采用**分层数据架构**：

```
┌─────────────────────────────────────────┐
│  Layer 1: 主词库 (ECDICT SQLite)         │
│  - 324万词，含音标/释义/词频/考试标签      │
│  - 用途：基础查询、过滤考试词汇           │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  Layer 2: 词根分析 (Ceelog GPT4 JSON)    │
│  - 8000核心词，详细词根拆解/助记法        │
│  - 用途：AI 生成卡牌时的参考数据          │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  Layer 3: 深度词源 (etymology-db CSV)    │
│  - 420万词源关系，多语言追溯              │
│  - 用途：构建词根族谱、同根词网络          │
└─────────────────────────────────────────┘
```

---

## 📦 详细资源列表

### 1. ECDICT - 主词库 ⭐⭐⭐⭐⭐ 必选

**GitHub**: https://github.com/skywind3000/ECDICT  
**Ultimate 版**: https://github.com/skywind3000/ECDICT-ultimate

**收词量**: 
- 基础版：324 万+
- Ultimate：432 万+

**格式**: SQLite (.db)、CSV、StarDict、MDX

**SQLite Schema**:
```sql
CREATE TABLE stardict (
    id INTEGER PRIMARY KEY,
    word TEXT UNIQUE,
    sw TEXT,           -- 词干（如 goes → go）
    phonetic TEXT,     -- 音标
    definition TEXT,   -- 英文释义
    translation TEXT,  -- 中文翻译
    pos TEXT,          -- 词性
    collins INTEGER,   -- 柯林斯星级（0-5）
    oxford INTEGER,    -- 是否牛津3000词（0/1）
    tag TEXT,          -- 考试标签："zk gk ky ielts toefl gre"
    bnc INTEGER,       -- 英国国家语料库词频
    frq INTEGER,       -- 当代语料库词频
    exchange TEXT,     -- 时态变形："p:went/d:gone/i:going/3:goes"
    detail TEXT,       -- 详细信息
    audio TEXT         -- 发音音频
);
```

**License**: 未明确声明，但广泛用于开源软件（GoldenDict、欧陆词典）

**下载**:
```bash
# 方式1: GitHub Release（推荐）
wget https://github.com/skywind3000/ECDICT/releases/download/1.0.28/ecdict-sqlite-28.zip

# 方式2: 直接克隆（较大）
git clone https://github.com/skywind3000/ECDICT.git
```

**为什么选它**:
- ✅ SQLite 格式，Rust `sqlx` 直接查询
- ✅ 含 `tag` 字段，直接过滤托福/GRE/雅思词汇
- ✅ 含词频（`bnc`/`frq`），可按常用度排序
- ✅ 含词干（`sw`），便于查找词根
- ✅ 324万词覆盖率极高

---

### 2. Ceelog/DictionaryByGPT4 - 词根分析 ⭐⭐⭐⭐⭐ 推荐

**GitHub**: https://github.com/Ceelog/DictionaryByGPT4

**收词量**: 8000 词（中考/高考/四六级核心）

**格式**: JSON + MDX + EPUB

**JSON 结构**:
```json
{
  "word": "abandon",
  "pronunciation": "/əˈbændən/",
  "pos": "v.",
  "meaning": "放弃，抛弃",
  "root_analysis": "a-(离开) + ban-(禁令) + -don(给予) → 不再禁令约束 → 放弃",
  "examples": [
    "They abandoned the project due to lack of funding.",
    "The ship was abandoned in the storm."
  ],
  "synonyms": ["desert", "forsake"],
  "memory_tip": "想象一个被禁令(ban)困住的人，离开(a-)后放弃(don)了束缚",
  "cultural_notes": "常用于正式场合，表示永久性放弃"
}
```

**License**: CC-BY-SA 4.0（署名 + 相同方式共享）

**下载**:
```bash
git clone https://github.com/Ceelog/DictionaryByGPT4.git
# JSON 文件在 data/ 目录
```

**为什么选它**:
- ✅ 专为中文学习者设计
- ✅ 每个词都有详细的词根拆解
- ✅ 含助记技巧和文化背景
- ✅ JSON 格式，直接解析
- ✅ 质量极高（GPT-4 生成 + 人工审核）

---

### 3. droher/etymology-db - 深度词源 ⭐⭐⭐⭐

**GitHub**: https://github.com/droher/etymology-db

**规模**: 
- 420万+ 词源关系
- 180万 词条
- 3300+ 语言

**格式**: Gzipped CSV + Parquet

**CSV Schema**:
```csv
word_id,word,language,etymology_type,source_word_id,source_word,source_language
1234,abandon,en,borrowed_from,5678,abandonner,fr
5678,abandonner,fr,inherited_from,9012,abandoner,la
9012,abandoner,la,compound,[root_a,root_bannum],...
```

**License**: CC ShareAlike 3.0（需署名，衍生作品同协议）

**下载**:
```bash
# 下载最新版本（约 200MB gzip）
wget https://github.com/droher/etymology-db/releases/latest/download/etymology.csv.gz
gunzip etymology.csv.gz
```

**为什么选它**:
- ✅ 最大规模的开源词源数据集
- ✅ 可追溯到拉丁语/希腊语词根
- ✅ 包含借词、继承等演变关系
- ✅ 可构建词根族谱

---

### 4. ImSingee/open-english-dictionary - 备选主词库 ⭐⭐⭐⭐

**GitHub**: https://github.com/ImSingee/open-english-dictionary

**收词量**: 41万单词 + 7.3万词组

**格式**: SQLite、NDJSON、MDX

**License**: MIT（明确可内嵌）

**下载**:
```bash
wget https://github.com/ImSingee/open-english-dictionary/releases/latest/download/open-english-dictionary.db
```

**为什么是备选**:
- ✅ License 清晰（MIT）
- ✅ 持续更新
- ⚠️ 收词量比 ECDICT 少（41万 vs 324万）
- ⚠️ 无考试标签字段

---

### 5. mahavivo/english-wordlists - 考试词表 ⭐⭐⭐

**GitHub**: https://github.com/mahavivo/english-wordlists

**包含**:
- `TOEFL.txt` (3000+ 词)
- `GRE_8000_Words.txt` (8000 词)
- `IELTS.txt` (4000+ 词)
- `CET4.txt` / `CET6.txt` (四六级)
- `COCA_abridged.txt` (美国当代英语语料库)

**格式**: TXT（每行：单词 + 音标 + 词性 + 中文释义）

**下载**:
```bash
git clone https://github.com/mahavivo/english-wordlists.git
```

**用途**: 
- 作为词书数据源
- 对比 ECDICT 的 `tag` 字段验证准确性

---

## 🔧 导入到项目的建议

### 方案 A: 最小化启动（推荐）

**只用 ECDICT**:
```
1. 下载 ecdict-sqlite-28.zip
2. 解压得到 stardict.db
3. 放到 src-tauri/resources/ecdict.db
4. sqlx 直接查询
```

**优点**:
- 单文件（约 200MB）
- SQLite 直接查询，无需解析
- 含基础词根信息（`sw` 词干字段）

**缺点**:
- 词根分析不够详细
- 需要 LLM 生成助记法

---

### 方案 B: 完整功能（推荐上线前）

**ECDICT + Ceelog JSON + etymology-db CSV**:

```
src-tauri/resources/
├── ecdict.db              # 主词库（200MB）
├── gpt4_dict_8000.json    # 词根分析（~5MB）
└── etymology.db           # 词源关系（自行导入 CSV 到 SQLite）
```

**导入步骤**:

1. **ECDICT**（直接用）
   ```bash
   wget https://github.com/skywind3000/ECDICT/releases/download/1.0.28/ecdict-sqlite-28.zip
   unzip ecdict-sqlite-28.zip
   mv stardict.db src-tauri/resources/ecdict.db
   ```

2. **Ceelog JSON**（解析到内存或导入 SQLite）
   ```bash
   git clone https://github.com/Ceelog/DictionaryByGPT4.git
   cp DictionaryByGPT4/data/*.json src-tauri/resources/
   ```

3. **etymology-db**（CSV 导入 SQLite）
   ```bash
   wget https://github.com/droher/etymology-db/releases/latest/download/etymology.csv.gz
   gunzip etymology.csv.gz
   
   # 用 Rust 或 Python 脚本导入到 SQLite
   # CREATE TABLE etymology (
   #     word TEXT,
   #     language TEXT,
   #     etymology_type TEXT,
   #     source_word TEXT,
   #     source_language TEXT
   # );
   ```

---

## 📊 数据库设计建议

```sql
-- 主词库（直接用 ECDICT 的 stardict 表）

-- 词根分析缓存（从 Ceelog JSON 导入）
CREATE TABLE word_root_analysis (
    word TEXT PRIMARY KEY,
    root_analysis TEXT,      -- "a-(离开) + ban-(禁令) + -don(给予)"
    memory_tip TEXT,
    examples TEXT,           -- JSON array
    cultural_notes TEXT
);

-- 词源关系（从 etymology-db CSV 导入）
CREATE TABLE etymology_relations (
    id INTEGER PRIMARY KEY,
    word TEXT,
    language TEXT,
    etymology_type TEXT,     -- borrowed_from | inherited_from | compound
    source_word TEXT,
    source_language TEXT,
    INDEX idx_etymology_word (word)
);

-- 用户卡牌（事件驱动，从 cards 表查询时 JOIN 以上表）
```

---

## 🚀 开发时的优先级

### Phase 1: 基础功能（1周内）
- [x] 只用 ECDICT SQLite
- [ ] DictionarySkill 查询基础信息
- [ ] 前端显示：音标、释义、词性

### Phase 2: 词根分析（2周内）
- [ ] 解析 Ceelog JSON 导入 SQLite
- [ ] 查询词根拆解数据
- [ ] 前端显示词根分析

### Phase 3: 深度词源（1个月内）
- [ ] 导入 etymology-db CSV
- [ ] 构建同根词网络
- [ ] 前端显示词源树

---

## 📝 License 总结

| 数据源 | License | 商业使用 | 需署名 | 相同方式共享 |
|--------|---------|----------|--------|-------------|
| ECDICT | 未明确 | ⚠️ 灰色地带 | - | - |
| ImSingee/open-english-dictionary | MIT | ✅ 可以 | ❌ 不需要 | ❌ 不需要 |
| Ceelog/DictionaryByGPT4 | CC-BY-SA 4.0 | ✅ 可以 | ✅ 需要 | ✅ 需要 |
| etymology-db | CC-SA 3.0 | ✅ 可以 | ✅ 需要 | ✅ 需要 |

**推荐策略**:
- 开源项目：全部可用
- 闭源/商业：用 ImSingee（MIT）替换 ECDICT，或联系 ECDICT 作者获得授权

---

## 🔗 下载脚本

```bash
#!/bin/bash
# 下载所有词典资源

mkdir -p dictionaries

# 1. ECDICT
echo "下载 ECDICT..."
wget https://github.com/skywind3000/ECDICT/releases/download/1.0.28/ecdict-sqlite-28.zip
unzip ecdict-sqlite-28.zip -d dictionaries/
mv dictionaries/stardict.db dictionaries/ecdict.db

# 2. Ceelog GPT4 词典
echo "克隆 Ceelog GPT4 词典..."
git clone --depth 1 https://github.com/Ceelog/DictionaryByGPT4.git dictionaries/gpt4-dict

# 3. Etymology DB
echo "下载 etymology-db..."
wget https://github.com/droher/etymology-db/releases/latest/download/etymology.csv.gz
gunzip etymology.csv.gz
mv etymology.csv dictionaries/

echo "✅ 词典下载完成！"
echo "位置: $(pwd)/dictionaries/"
```

---

## 📚 参考资源

- [ECDICT GitHub](https://github.com/skywind3000/ECDICT)
- [Ceelog/DictionaryByGPT4](https://github.com/Ceelog/DictionaryByGPT4)
- [droher/etymology-db](https://github.com/droher/etymology-db)
- [ImSingee/open-english-dictionary](https://github.com/ImSingee/open-english-dictionary)
- [mahavivo/english-wordlists](https://github.com/mahavivo/english-wordlists)
