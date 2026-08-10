# Spike Verdict — Firefox/Bergamot CJK 模型作为原生 C++ 后端

Date: 2026-08-10
Machine: Windows x64, VS 2022 BuildTools (MSVC 14.43), cmake 3.30.6, ninja 1.13, Python 3.13.2

## Objective

验证 Mozilla Firefox 生产的 CJK（en-zh / zh-en）Marian intgemm 模型能否被
**原生 C++ bergamot-translator v0.4.4** 加载并双向翻译。这是选定的首选离线引擎档位
（~50MB 级、MPL 2.0、CPU int8、无 GPU/LLVM 运行时依赖）。

## Result: ✅ VERIFIED

| 方向 | 模型文件 | 大小 | 加载 | 翻译验证 |
|---|---|---|---|---|
| en→zh | model.enzh.intgemm.alphas.bin + srcvocab/trgvocab.enzh.spm + lex.50.50.enzh.s2t.bin | 43.85 MB (bin) | ✅ | ✅ `你好,这是对Firefox翻译引擎的一次测试。` |
| zh→en | model.zhen.intgemm.alphas.bin + vocab.zhen.spm + lex.50.50.zhen.s2t.bin | 59.50 MB (bin) | ✅ | ✅ `Hello world. This is a test of the Firefox translation engine.` |

模型来源：Mozilla Model Registry `https://storage.googleapis.com/moz-fx-translations-data--303e-prod-translations-data/db/models.json`
（en-zh/llmaat_finetune10M_qe8_f2_…、zh-en/cjk_icu_base_LQeOIbF7…），SHA-256 与 registry 匹配。
模型是 Marian intgemm8 量化（CPU int8），许可证 MPL 2.0（商用友好）。

## Windows 原生构建（本 spike 首次验证，CI 无 Windows 先例）

bergamot-translator v0.4.4 (5ae1b1e) 从源码构建 `bergamot` CLI 成功，需如下补丁
（均为本地临时改动，不进上游；正式架构文档需记录）：

1. **protobuf-lite 老版 hash.h 对现代 MSVC 不兼容**（sentencepiece 内嵌 protobuf-lite）：
   - `stdext::hash_compare` 已在 VS2022 移除 → 为 MSVC>=1600 分支注入基于
     `std::hash` + `std::less` 的 `google::protobuf::hash_compare` shim
     （`3rd_party/marian-dev/src/3rd_party/sentencepiece/third_party/protobuf-lite/google/protobuf/stubs/hash.h`）。
2. **C4819 编码警告被当错误**（pathie-cpp 头文件为 UTF-8，当前代码页 936）：
   - marian CMakeLists `DISABLE_GLOBALLY` 追加 `/wd4819`；CMAKE_CXX_FLAGS 追加 `/utf-8`。
3. **ssplit-cpp 内部 pcre2 在 MSVC 输出 `pcre2-8-static.lib`** 而 FindPCRE2.cmake 期望 `pcre2-8.lib`：
   - FindPCRE2.cmake 按 MSVC 分支修正库名；ssplit 目标编译时定义 `PCRE2_STATIC`
     （否则 pcre2 头按 dllimport 声明导致 `__imp_pcre2_*` 链接错误）。
4. **marian 默认要求 BLAS/MKL，无则 ABORT**：
   - 用 marian 自带 `USE_WASM_COMPATIBLE_BLAS=ON`（onnxjs 纯 C++ Eigen GEMM），
     同时需 `USE_WASM_COMPATIBLE_SOURCE=ON` 才能绕过 CMAKE_DEPENDENT_OPTION 依赖。
     无需外部 BLAS 库。
5. **cmake 版本**：系统 cmake 4.2.3 与 2018 年代子项目不兼容，需便携 3.30.6
   加 `-DCMAKE_POLICY_VERSION_MINIMUM=3.5`。

构建配置要点：
```
cmake -G "Visual Studio 17 2022" -A x64 -DCMAKE_CONFIGURATION_TYPES=Release \
  -DSSPLIT_USE_INTERNAL_PCRE2=ON -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -DUSE_WASM_COMPATIBLE_BLAS=ON -DUSE_WASM_COMPATIBLE_SOURCE=ON -DUSE_MKL=OFF \
  -DCMAKE_CXX_FLAGS="/DWIN32 /D_WINDOWS /W3 /GR /EHsc /utf-8"
```

## CJK 模型 config 组装

CJK 模型无自带 marian config.yml（registry 只提供 model/vocab/lex 文件），
需手写基础 marian config（models / vocabs / shortlist + ssplit-mode、max-length-break、
mini-batch-words、alignment: soft、max-length-factor 2.0），路径自动相对于 config 文件解析。

## Pivot 链路验证（日⇄中 / 俄⇄中 / 韩⇄中 via 英语）

Registry 只有 `X→en` / `en→X` 星形结构（111 对），中文相关仅
`en-zh / zh-en / en-zh_hant / zh_hant-en`。**日、俄、韩对中文无直接对，须经英语 pivot**。
已下载 ja-en / en-ja（Desktop Release 档，SHA-256 与 registry 匹配）实测：

| 链路 | 中间结果 (en) | 最终结果 | 结论 |
|---|---|---|---|
| ja→en→zh `月が明るい。` | The moon is bright. | 月亮是明亮的。 | ✅ |
| ja→en→zh `今日は天気が良いですね。` | The weather is good today, isn't it? | 今天天气好了,不是吗? | ✅ |
| ja→en→zh `私は東京に住んでいます。` | I live in Tokyo. | 我住在东京。 | ✅ |
| zh→en→ja `今天的月亮非常明亮。` | The moon is very bright today. | 今日は月がとても明るい。 | ✅ |

实现方式：两次调用（pivot 中间结果经英语中转）。CLI 的 `--model-config-paths` 支持
多模型/多工作流，但 pivot 链路由应用层串联两个翻译服务更可控。日/俄/韩↔中
各需 3 个模型（X↔en + en↔zh），模型尺寸与 en-zh 同档位（~44–60MB 每个）。

新增模型文件（config.jaen.yml / config.enja.yml + 解压 bin/spm/lex），SHA-256 已验证。

## Runtime notes（实测，4 CPU threads）

| 指标 | en-zh | zh-en |
|---|---|---|
| 加载后内存（未翻译，工作集） | ~67 MB | ~92 MB |
| 单句全流程（load+translate） | 520–730 ms | ~390 ms |
| 10 句小批峰值工作集 | ~352 MB | ~92 MB |
| 稳态吞吐（200 句含加载） | ~30 句/s | — |

说明：CLI 一次读入全部 stdin 形成大 batch 时峰值内存虚高（50 句 → 工作集 600MB / 私有 3.5GB 保留）；
真实应用按 1–N 句小批调用时工作集在 100–350MB 量级。私有 RSS 显著高于工作集，主要是
marian/onnxjs 的未触达虚拟保留，非实际提交。

## Conclusion for catalog

- ✅ **Firefox/Bergamot CJK 是可行且推荐的首选默认引擎**：~50MB 模型、CPU int8、MPL 2.0、
  双向中英翻译质量可用（Firefox 生产级）。
- 集成形态建议：原生 C++ 静态链接 marian/bergamot-translator，包体增加约
  bergamot-translator 库 + 模型 44MB(zh) 或双向双模型 ~103MB。可只捆绑 en-zh + zh-en 单模型按需下载。
- Windows 构建补丁清单（上文 5 项）需固化到正式架构的构建脚本/文档，防止回归。

---

# 历史：Hy-MT GGUF loadability via llama_cpp（已否决路线，存档）

Date: 2026-08-10
Machine: Windows x64, Rust stable-x86_64-pc-windows-msvc, LLVM 22.1.8
Crate: llama_cpp 0.1.3 / llama_cpp_sys 0.2.2 (builds llama.cpp from source, static link)

## 结论（否决）

两条官方 GGUF（Hy-MT1.5-1.8B-1.25bit / 2bit）均无法被主线 llama.cpp 加载：
`gguf_init_from_file: tensor 'blk.0.attn_k.weight' ... not a multiple of block size`
（1.25bit 用 Sherry STQ1_0，PR #22836 未合并；2bit 用 AngelSlim SEQ "coming soon"）。
自量化 Q4_K_M（~1.08GB）路线因 CJK/bergamot 方案胜出而不再需要。

## 遗留结论（供参考）

- 若未来需要 1B+ 高质量 MT，可重估 tencent/HY-MT1.5-1.8B 自量化（Q4_K_M）。
- 本 spike 的 Cargo/llama_cpp 测试代码保留在 spike/ 目录作历史参考。
