# 非常规 / 免配置翻译引擎

本波新增的「免配置」引擎走网页或非官方端点，**可能随时失效**。设置页默认关闭。

| id | 名称 | 端点来源 | 说明 |
|----|------|----------|------|
| `youdao` | 有道（已有） | 网页 | 非开放平台 AppKey |
| `baidu_web` | 百度免配置 | `fanyi.baidu.com/transapi` | 与官方 VIP `baidu` 并存 |
| `caiyun_web` | 彩云免配置 | JWT + 浏览器 token | 正式请用 `caiyun` + Token |
| `volcengine_web` | 火山免配置 | `translate.volcengine.com/crx/...` | Luna huoshan |
| `transmart` | 腾讯交互 | `transmart.qq.com/api/imt` | pot transmart；覆盖「腾讯交互」 |
| `papago` | Papago | papago.naver.com | Luna papago |
| `tatoeba` | Tatoeba | tatoeba.org API | **例句**，不是机翻 |

**本波不做：** 腾讯君 TMT 云 API 正规 Key、TMT 免配置（无稳定端点则跳过，用 Transmart 代替交互场景）。

失败时 Router 按策略回退，不会拖垮其它引擎。
