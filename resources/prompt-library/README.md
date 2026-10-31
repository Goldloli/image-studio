# 提示词灵感库资源

`catalog.json` 由 `scripts/import_prompt_library.py` 从以下固定快照生成：

- 仓库：`freestylefly/awesome-gpt-image-2`
- 提交：`63e6176fa01be51c000108dacd72716d3e46784b`
- 数据：`data/cases.json`、`data/images/`

目录包含 541 条案例和 13 个主分类。示例图片不随应用打包；目录只保存固定来源、大小和 SHA-256，由桌面宿主按登记案例 ID 按需下载、校验并缓存。

上游仓库许可证为 MIT，但第三方示例图片与提示词的商业使用、再分发条件不能由该许可证统一推定。应用始终显示来源，且不把案例描述为本机生成或已在本机验证。

重新导入：

```bash
python3 scripts/import_prompt_library.py \
  --source-root /path/to/fixed-upstream \
  --images-root /path/to/fixed-upstream-images \
  --corrections resources/prompt-library/manual-corrections.json \
  --output-root resources/prompt-library
python3 -m unittest scripts.tests.test_import_prompt_library
```

人工复核结论写入 `manual-corrections.json`，重新导入不会覆盖。允许字段为 `inputKind`、`note` 和 `reviewStatus`。
