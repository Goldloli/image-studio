# 灵感库实施验收结果

日期：2026-10-31
实现检查点：`8b1fca1`
环境：macOS 27.0.1 · arm64 · Apple M3 Max · 128 GiB

## 结论

灵感库的目录导入、分类检索、详情与复制、按需图片、收藏、缓存、从案例新建会话、应用到当前草稿及真实生成路径均已通过。验收使用独立 macOS App 标识和隔离数据根；真实生成使用现有模型验证根的 APFS 副本，未写入默认用户库或其他既有验收根。

## 目录

- 固定上游提交 `63e6176fa01be51c000108dacd72716d3e46784b`。
- 541 个唯一案例、13 个主分类、26 个精选案例。
- 缺号仅为 12、169、170；25 条无原帖 URL 的案例保留来源署名和 GitHub 兜底入口。
- 541 个图片映射均有 JPEG/PNG 类型、尺寸、字节数和 SHA-256；总计 157,178,325 字节，最大单图 5,434,675 字节。
- 使用条件审计结果：481 条纯文生图、57 条参考图、3 条图片编辑；343 条含可替换变量。规则审计结果可通过独立修正表覆盖。
- `catalog.json` SHA-256：`c33355ca0dba6c0cc6d1078b101182be359549824a1786b9eee9e1b4dad1deff`。

## 自动与浏览器验证

- `python3 -m unittest scripts.tests.test_import_prompt_library`：3/3 通过。
- `npm run check`：通过。
- `npm test -- --run`：35/35 通过。
- `npm run build`：通过；目录作为独立懒加载 chunk，不阻塞主界面首包。
- `cargo test --workspace --locked --no-fail-fast`：96 通过、0 失败、3 个既有显式验收测试忽略。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `openspec validate introduce-prompt-inspiration-library --strict`：通过。
- Playwright 浏览器流程：541 卡片、全文搜索、详情、完整提示词复制、收藏、新建会话、当前草稿替换、窄窗布局和详情焦点循环均通过。
- 宿主真实网络用例：从固定来源下载案例 544 后校验散列和缓存读取通过。

## 原生验证

- 首次无法联网时，目录文字、搜索、详情、收藏与草稿应用可用，图片显示可见失败占位。
- 联网后首屏图片按可见区域加载，缓存从 0 增长到 1.7 MB；关闭并重启后 1 个收藏仍在。
- 清理缓存后按钮从 1.7 MB 回到 0 KB，收藏和案例草稿保持不变；随后使用失效代理重启，已缓存的首屏图片仍可离线读取。
- 1280×800 浅色与 960×640 深色布局可用，详情主要动作均可达；模态 Tab 焦点保持在对话框内。
- 从案例 510「Bichon Shop 拟物 App 图标」创建会话时只填入原文，不自动发送，并保持 Z-Image Turbo。
- 将另一条案例应用到已有草稿时显示替换确认；确认后完整替换，一次撤销恢复原草稿。
- 真实生成期间切换到灵感库并浏览，任务继续从 4/9 推进到 6/9。
- 显式回车发送后完成 9/9 采样、解码和原子提交：
  - Job：`job-4001f665275e4360bec57274d54ee847`
  - Turn：`turn-afcd4b130e6c460bacf10fb217bd1db4`
  - Attempt：`attempt-594b8615d87442d294bc506bc29a671f`，状态 `succeeded`
  - Asset：`asset-19289eeed39c4b208eecdd3b650b3f24`
  - PNG：1024×1024 RGB，624,648 字节
  - SHA-256：`d86ed8d20c429883014691c6b06fcbdcfeeeec8e3b8dd1aefb62df53166dbde2`
- 源验证库为 10 个 Job、7 个资产；只显式发送一次后隔离副本为 11 个 Job、8 个资产，浏览和填词本身没有增加任务或作品。

## 证据

- [原生 1280 浅色灵感库](./native-1280-light-library.png)
- [原生 960×640 深色灵感库](./native-960-dark-library.png)
- [原生案例详情](./native-1280-detail-light.png)
- [原生案例会话草稿](./native-1280-created-conversation.png)
- [真实生成期间结果](./native-1280-generated-from-case.png)
- [断网读取已缓存图片](./native-1280-offline-cached-images.png)
- [真实生成 PNG](./native-generated-bichon-shop.png)
- [浏览器 1280 灵感库](./browser-1280-library.png)
- [浏览器案例详情](./browser-1280-detail.png)
- [浏览器 960 灵感库](./browser-960-library.png)

## 已知边界

- 示例图片首次查看需要联网；清空缓存且断网时显示明确占位，不影响文字、收藏和草稿。
- 卡片使用原始示例图而非派生缩略图，图片只在进入可见区域时请求。
- 上游未提供逐案例可信模型、种子、尺寸或生成参数，本应用不承诺复现示例；真实生成结果与上游图片不同属于预期。
- 第三方图片和提示词的商业使用及再分发权利未统一验证，界面保留来源入口和不可商用声明。
