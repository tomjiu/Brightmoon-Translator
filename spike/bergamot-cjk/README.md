# Firefox/Bergamot CJK 模型 Spike 产物

Task 1 Spike：验证 Mozilla Firefox 生产 CJK Marian intgemm 模型能否作为原生 C++ 后端。
结论详见 `../VERDICT.md`（VERIFIED）。

## 文件

- `config.enzh.yml` / `config.zhen.yml` / `config.jaen.yml` / `config.enja.yml` — 手写基础
  marian config（CJK 模型无自带 config）
- `*.bin.gz` / `*.spm.gz` — en-zh / zh-en / ja-en / en-ja 模型
  （gz 压缩态，解压后为 marian 直接加载格式）

模型清单（SHA-256 与 Mozilla Model Registry 匹配，来源见 VERDICT.md）：

| 方向 | 解压后文件 | 大小 |
|---|---|---|
| en→zh | model.enzh.intgemm.alphas.bin | 43.85 MB |
| en→zh | srcvocab.enzh.spm / trgvocab.enzh.spm | 0.8 MB |
| en→zh | lex.50.50.enzh.s2t.bin | 4.5 MB |
| zh→en | model.zhen.intgemm.alphas.bin | 59.50 MB |
| zh→en | vocab.zhen.spm | 1.4 MB |
| zh→en | lex.50.50.zhen.s2t.bin | 9.2 MB |
| ja→en | model.jaen.intgemm.alphas.bin | 59.50 MB |
| ja→en | vocab.jaen.spm / lex.50.50.jaen.s2t.bin | 1.4 MB / 9.3 MB |
| en→ja | model.enja.intgemm.alphas.bin | 43.85 MB |
| en→ja | srcvocab.enja.spm / trgvocab.enja.spm | 0.8 MB |
| en→ja | lex.50.50.enja.s2t.bin | 4.5 MB |

日⇄中 / 俄⇄中 / 韩⇄中无直接模型，经英语 pivot 两段翻译（见 VERDICT.md「Pivot 链路验证」）。

## 复现

```bash
# 1) 解压模型
gunzip -k model.enzh.intgemm.alphas.bin.gz srcvocab.enzh.spm.gz trgvocab.enzh.spm.gz lex.50.50.enzh.s2t.bin.gz

# 2) 用构建好的 bergamot CLI（Windows 构建补丁见 VERDICT.md）
echo "Hello world." | bergamot --model-config-paths config.enzh.yml --cpu-threads 4
# → 你好,世界。
```

构建好的二进制位于（临时工作区）：
`C:\Users\yezi6\AppData\Local\Temp\opencode\bergamot-translator\build-native\app\Release\bergamot.exe`
