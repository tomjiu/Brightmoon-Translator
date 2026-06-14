# 词典资源清单 - 已下载

生成时间: 2026-06-14
状态: **所有资源已就位**

---

## ✅ 合规开源词典（可用于软件发布）

### 1. ECDICT - 主词库 ⭐⭐⭐⭐⭐
- **文件**: `ecdict.db`
- **大小**: 812MB
- **格式**: SQLite
- **收词**: 324万词
- **内容**: 音标、释义、词频、考试标签（toefl/ielts/gre）、词干、时态变形
- **License**: 社区开源（广泛使用，但未明确声明）
- **用途**: 基础词义查询、考试词汇筛选

---

### 2. MorphoLex - 词根拆解数据库 ⭐⭐⭐⭐⭐
- **文件**: `morpholex/MorphoLEX_en.xlsx`
- **大小**: 6.5MB
- **格式**: Excel（需转 CSV）
- **收词**: 70,000 英语单词
- **内容**: **前缀.词根.后缀** 专业拆解（如 `archi.tect.ure`）
- **来源**: 学术论文（专家标注）
- **License**: 学术开源
- **用途**: 词根词缀教学、单词拆解分析

**示例数据**:
```
Word          | MorphoLexSegm  | MorphoLexPOS
abandon       | a.ban.don      | V
architecture  | archi.tect.ure | N
brilliant     | brill.i.ant    | ADJ
```

---

### 3. Oxford 41K - 牛津词典 ⭐⭐⭐⭐
- **文件**: `oxford-41k/oedict.sql`
- **大小**: 5.2MB
- **格式**: SQL（可导入 SQLite）
- **收词**: 41,000+ 词
- **来源**: 牛津英语词典
- **License**: 开源（社区整理）
- **用途**: 高质量释义（比 ECDICT 翻译质量更好）

---

### 4. Etymology-DB - 词源关系数据库 ⭐⭐⭐⭐⭐
- **文件**: `etymology.csv.gz`
- **大小**: 137MB（压缩）
- **格式**: CSV（gzip 压缩）
- **规模**: 420万词源关系，180万词条，2900种语言
- **来源**: Wiktionary 解析
- **License**: CC ShareAlike 3.0
- **用途**: 词源追溯（英语 → 法语 → 拉丁语）、同源词网络

**关系类型**:
- borrowed_from（借词）
- inherited_from（继承）
- compound（复合词）
- 等 31种关系

---

### 5. Wiktionary StarDict - 维基词典 ⭐⭐⭐
- **目录**: `wiktionary-stardict/`
- **来源**: Wiktionary 官方数据转换
- **License**: CC-BY-SA（维基百科协议）
- **用途**: 开源词典参考、词源验证

---

## ⚠️ 个人研究词典（不可商用）

### 6. Ceelog GPT4 Dictionary - AI 生成词典
- **文件**: `gpt4-dict/gptwords.json`
- **大小**: 17MB
- **格式**: JSON
- **收词**: 8,000 核心词（中考/高考/四六级）
- **来源**: GPT-4 生成 + 人工审核
- **License**: CC-BY-SA 4.0
- **用途**: 参考词根分析、助记法、文化背景
- **风险**: ⚠️ AI 幻觉风险，需验证
- **状态**: **仅个人学习研究，不可用于软件发布**

**内容**:
- 词根词缀拆解（文字描述）
- 助记法（联想/谐音/词根）
- 文化背景（如 March → 战神 Mars）
- 例句（3个情景）
- 记忆小故事

---

## 📊 数据对比

| 词典 | 词汇量 | 词根拆解 | 词源追溯 | 释义质量 | 合规性 |
|------|--------|---------|---------|---------|--------|
| **ECDICT** | 324万 | ❌ | ❌ | ⭐⭐⭐ | ✅ |
| **MorphoLex** | 7万 | ✅✅✅✅✅ | ❌ | - | ✅ |
| **Oxford 41K** | 4.1万 | ❌ | ❌ | ⭐⭐⭐⭐⭐ | ✅ |
| **Etymology-DB** | 180万 | ❌ | ✅✅✅✅✅ | - | ✅ |
| **Wiktionary** | 数百万 | ⚠️ | ✅✅✅ | ⭐⭐⭐⭐ | ✅ |
| **Ceelog GPT4** | 8千 | ⚠️⚠️⚠️ | ⚠️⚠️ | ⭐⭐⭐⭐ | ❌ |

---

## 🎯 推荐使用策略

### 软件发布版本（合规）

```
Layer 1: 基础查询
  └─ ECDICT (324万词)
      用途: 音标、释义、词频

Layer 2: 词根拆解
  └─ MorphoLex (7万词)
      用途: archi.tect.ure 专业拆解

Layer 3: 词源追溯
  └─ Etymology-DB (180万词)
      用途: 追溯到拉丁/希腊语

Layer 4: 高质量释义（可选）
  └─ Oxford 41K
      用途: 替代 ECDICT 的低质量翻译
```

### 个人研究版本（含 AI）

```
+ Ceelog GPT4 (8千词)
  用途: 助记法参考、文化背景
  标注: "AI 生成，仅供参考"
```

---

## 📁 文件结构

```
dictionaries/
├── ecdict.db                     # 812MB, SQLite
├── ecdict-sqlite-28.zip          # 原始压缩包
├── morpholex/
│   └── MorphoLEX_en.xlsx         # 6.5MB, 需转 CSV
├── oxford-41k/
│   └── oedict.sql                # 5.2MB, SQL
├── etymology.csv.gz              # 137MB, CSV 压缩
├── wiktionary-stardict/          # StarDict 格式
└── gpt4-dict/
    └── gptwords.json             # 17MB, JSON
```

**总大小**: 约 1GB

---

## 🔧 待处理任务

### 数据格式转换

1. **MorphoLex XLSX → CSV**
   ```bash
   # 需要 pandas + openpyxl
   pip install pandas openpyxl
   python -c "
   import pandas as pd
   df = pd.read_excel('morpholex/MorphoLEX_en.xlsx')
   df.to_csv('morpholex/MorphoLEX_en.csv', index=False)
   "
   ```

2. **Etymology-DB 解压**
   ```bash
   gunzip etymology.csv.gz
   # 得到 etymology.csv
   ```

3. **Oxford 41K 导入 SQLite**
   ```bash
   sqlite3 oxford-41k.db < oxford-41k/oedict.sql
   ```

---

## 📋 集成优先级

### Phase 1（当前）
- ✅ ECDICT SQLite（基础查询）

### Phase 2（2周内）
- [ ] MorphoLex CSV（词根拆解）
- [ ] Oxford 41K SQLite（高质量释义）

### Phase 3（1个月内）
- [ ] Etymology-DB CSV（词源追溯）

### 可选（后期）
- [ ] Wiktionary StarDict（在线验证）

---

## 🚨 License 说明

### 可商用
- ✅ MorphoLex（学术开源）
- ✅ Etymology-DB（CC-SA 3.0，需署名）
- ✅ Wiktionary（CC-BY-SA，需署名）

### 灰色地带
- ⚠️ ECDICT（未明确 License，但广泛使用）
- ⚠️ Oxford 41K（社区整理，来源不明）

### 不可商用
- ❌ Ceelog GPT4（个人研究，AI 生成）

**建议**:
- 开源项目：全部可用
- 商业闭源：谨慎使用 ECDICT/Oxford 41K，或联系作者授权

---

## 📚 参考链接

- [ECDICT GitHub](https://github.com/skywind3000/ECDICT)
- [MorphoLex GitHub](https://github.com/hugomailhot/MorphoLex-en)
- [MorphoLex 论文](https://link.springer.com/article/10.3758/s13428-017-0981-8)
- [Etymology-DB GitHub](https://github.com/droher/etymology-db)
- [Oxford 41K GitHub](https://github.com/DevangMstryls/Oxford-English-Dictionary-41K-words)
- [Wiktionary StarDict GitHub](https://github.com/xxyzz/wiktionary_stardict)
- [Ceelog GPT4 GitHub](https://github.com/Ceelog/DictionaryByGPT4)

---

**状态**: ✅ 所有词典资源已下载完成，可以开始 Phase 1 开发！
