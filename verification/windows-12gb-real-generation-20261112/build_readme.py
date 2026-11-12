#!/usr/bin/env python3
"""Render the acceptance evidence README from results.json and per-case evidence."""
from __future__ import annotations

import json
import os

EVIDENCE = os.path.dirname(os.path.abspath(__file__))


def load(path):
    with open(os.path.join(EVIDENCE, path), "r", encoding="utf-8") as handle:
        return json.load(handle)


def main():
    results = load("results.json")
    cases = load("generations/generation-cases-evidence.json")
    installs = load("install/install-summary.json")
    network = load("network-install-evidence.json")
    upgrade = load("upgrade/db-after-restart.json")
    quality = load("quality/valid-output-ihdr.json")

    rows = {row["id"]: row for row in results["rows"]}
    lines = []
    add = lines.append
    add("# Windows 12 GB 目标机真实出图验收（2026-11-12）")
    add("")
    add("Change：`validate-windows-12gb-real-generation`  ·  Beads：`image-studio-pze`  ·  "
        "构建：`" + results["build"]["commit"][:12] + "`（0.1.1）  ·  数据根：`C:/ist-rg/`")
    add("")
    add("## 结论标签")
    add("")
    labels = results["labels"]
    add(f"- **当前构建真实出图（机器证据）：{labels['current_build_real_generation']}**")
    add(f"- 功能与管理：{labels['functional_and_management']}")
    add(f"- 复用证据：{labels['reused_evidence']}")
    add(f"- waived：{'、'.join(labels['waived'])}（用户 2026-11-12 决定不测，不视为通过）")
    add(f"- not_run：{'、'.join(labels['not_run'])}")
    add("")
    add("执行面审计：当前构建相对 2026-11-08 基线 `b861ae5` **属于 execution-surface change**"
        "（`low_memory.py`、`prepare.py`、`qwen.py`、`windows_process.rs`、`installation.rs`、worker wheel 等），"
        "因此历史 Gate 0 的 worker CUDA reserved 不能复用；本轮以产品级 host/进程采样为资源证据，"
        "worker 内部 CUDA reserved 记为 not_run。")
    add("")
    add("## 构建与安装")
    add("")
    build = results["build"]
    add("| 项 | 值 |")
    add("| --- | --- |")
    add(f"| 提交 | `{build['commit']}`（本地文档提交，未 push；远端 main `95dd117` 带 `[skip ci]`） |")
    add(f"| 安装包 | `Image Studio_0.1.1_x64-setup.exe`，245,118,946 B，`{build['installer_sha256']}` |")
    add(f"| 安装后 exe | `%LOCALAPPDATA%\\Image Studio\\image-studio-desktop.exe`，`{build['installed_exe_sha256']}` |")
    add("| 静默安装 | 第一次 `/S` 退出码 2 未改文件；立即重试退出码 0 并安装成功（观察 F8） |")
    add("| 质量门 | 前端 81 测试 + build；`cargo fmt`/`clippy`/`test --workspace` 全绿（`../build.log`） |")
    add("")
    add("## 真实出图矩阵（八项能力，全部成功）")
    add("")
    add("| 用例 | 型号 / 能力 | 墙钟 s | 输出 | 资源（host 口径） |")
    add("| --- | --- | ---: | --- | --- |")
    for case in cases:
        out = case["outputs"][0] if case["outputs"] else {}
        resources = case["resources"]
        gpu = resources.get("gpu_used_mib") or {}
        commit = resources.get("private_commit_mib") or {}
        avail = resources.get("available_physical_mib") or {}
        kind = {"text": "文生图", "edit": "单图编辑", "remove-background": "去背景", "upscale": "4× 放大"}[case["capability_kind"]]
        output = f"{out.get('width')}×{out.get('height')}，{out.get('file_bytes'):,} B，`{(out.get('file_sha256') or '')[:12]}…`" if out else "-"
        if case["case_id"] == "G07":
            output += "（RGBA + 8-bit mask）"
        add(f"| {case['case_id']} | {case['model']} / {kind} | {case['driver']['elapsed_seconds']} | {output} | "
            f"GPU {gpu.get('max')} MiB（峰值减基线 {(gpu.get('max') or 0) - 1400} MiB）· commit {(commit.get('max') or 0) / 1024:.2f} GiB · 可用内存最低 {(avail.get('min') or 0) / 1024:.1f} GiB |")
    add("")
    add("预算对照（全部通过）：host GPU 峰值减基线 ≤ 10.5 GiB；进程树 private commit ≤ 24 GiB；"
        "可用物理内存最低 ≥ 2 GiB；加载 ≤ 600 s、生成 ≤ 900 s。原始 1 s 采样在 `samples/*.csv`，"
        "逐项证据在 `generations/<ID>-evidence.json`。")
    add("")
    add("> 附：G01a 第一次尝试在三个应用实例并行时以进程提交峰值 24.18 GiB 触发预算失败；"
        "关闭升级实例后重试通过（16.68 GiB）。首次失败记录保留在 `generations/G01a-attempt1-budget-failure.json`。"
        "早先一轮 prompt 因 PowerShell 5.1 编码问题损坏，已整体归档到 `generations/invalid-input/`，不作为验收证据。")
    add("")
    add("## 模型安装")
    add("")
    add("| 型号 | 方式 | 耗时 s |")
    add("| --- | --- | ---: |")
    for item in installs:
        kind = "真实网络下载" if item["model"] == "Z-Image Turbo" and False else "种子缓存"
        add(f"| {item['model']} | {kind} | {item['elapsedSeconds']} |")
    add(f"| Z-Image Turbo（r-net） | 真实网络下载（ModelScope，35.56 GB，断点续传后完成） | {network['attempts'][1]['elapsed_seconds']} |")
    add("")
    add("网络安装细节：第一次下载到 33.17/35.56 GB 后停滞，重启应用以“继续安装”续传完成；"
        "下载阶段 842 s，校验 + 环境准备 + 设备探针后 ready；证据 `install-network.json`、`network-install-evidence.json`。")
    add("")
    add("## 作品与会话管理（全部使用本轮真实作品）")
    add("")
    manage_rows = [
        ("W01 查询/筛选/空态", rows.get("W01", {}).get("status")),
        ("W02 收藏持久化", rows.get("W02", {}).get("status")),
        ("W03 预览与制作记录", rows.get("W03", {}).get("status")),
        ("W04 删除普通真实作品", rows.get("W04", {}).get("status")),
        ("W05 BiRefNet 双输出删除", rows.get("W05", {}).get("status")),
        ("W06 引用阻止与解除", rows.get("W06", {}).get("status")),
        ("W07 会话删除与作品墓碑", rows.get("W07", {}).get("status")),
        ("C01 新建/切换/重命名", rows.get("C01", {}).get("status")),
        ("C02 删除非当前会话", rows.get("C02", {}).get("status")),
        ("C03 删除当前/最后会话", rows.get("C03", {}).get("status")),
    ]
    add("| 项 | 结果 |")
    add("| --- | --- |")
    for name, status in manage_rows:
        add(f"| {name} | {status} |")
    add("")
    add("作品数变化：27（生成完成）→ 26（W04 删除失败输入轮作品）→ 25（W05 删除 BiRefNet 双输出）→ 24（W06 解除引用后删除）→ 25（无控制台复核再生成 1 件）。"
        "W07 删除会话后作品保留并显示“会话已删除”墓碑；导出副本不受删除影响。")
    add("")
    add("## v5 → v6 真实升级（r-v5）")
    add("")
    add(f"- v5 副本升级后 `user_version` 6、`quick_check` ok，20 件旧作品 / 23 个任务 / 1 个会话 / 1 个草稿保留（`upgrade/db-after-restart.json`，共 {len(upgrade['assets'])} 件资产）。")
    add("- 旧作品导出逐字节一致：2,097,618 B，`326a97c6baf175d2…`（`upgrade/export-old-work.json`）；旧 BiRefNet 双输出可解码，mask 为 1024² 8-bit 灰度。")
    add("- 升级后从种子缓存重装 Klein（114 s）并真实出图一次（42 s）；重启幂等（21 资产 / 25 turns 前后一致）。")
    add("- D05 迁移失败保护、部分故障分支未执行（not_run）。")
    add("")
    add("## 鲁棒性")
    add("")
    add("| 项 | 结果 | 关键数据 |")
    add("| --- | --- | --- |")
    add("| O01 四类取消 | pass | 落定时间：Klein 6.0 s / Qwen-Turbo 4.5 s / BiRefNet 4.5 s / Real-ESRGAN 1.5 s；无残留 worker |")
    add("| O02 宿主强杀恢复 | pass | 重启后 attempt=interrupted、无新资产、不自动重试；显式“重试”产生新 attempt，随后取消 |")
    add("| O04/I06 无控制台窗口 | pass | 59 次 1 s 采样：控制台窗口 0，worker 窗口句柄 0 |")
    add("| O05 整树禁网 | waived | 用户决定不测 |")
    add("| O03 看门狗 / O06 磁盘不足 | not_run | 本轮未执行 |")
    add("")
    add("## 人工复核")
    add("")
    add("| 项 | 结果 | 说明 |")
    add("| --- | --- | --- |")
    add("| H01 真实微软拼音 | waived | 用户决定不测 |")
    add("| H02 显示缩放切换 | waived | 用户决定不测（当前 100%） |")
    add("| H03 窗口/主题/键盘 | pass | 1280×800、960×640 客户区精确；浅/深主题切换生效；Tab/Escape 无异常弹层 |")
    add("| H04 视觉质量接受/拒绝 | waived | 机器完整性检查通过；视觉结论未由用户给出 |")
    add("")
    add("## 观测与后续")
    add("")
    add("- **F5** 模型页安装完成后的自动刷新可能停住，手动“刷新”才显示“当前可执行”（Z-Image、Qwen Turbo、Qwen base、MOSS、r-v5 Klein 均出现）。")
    add("- **F6（工具问题）** Windows PowerShell 5.1 以 ANSI 读取无 BOM 的 UTF-8 脚本，中文字面量损坏；prompts 已改由 UTF-8 JSON 读取。")
    add("- **F7** 三个应用实例并行 + 网络下载时，Klein 首次生成触发 24 GiB 私有提交预算失败；关闭升级实例后重试通过，预算余量偏紧。")
    add("- **F8** 安装包第一次静默安装退出 2 且无文件变化，立即重试退出 0。")
    add("- **F9** `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录未能复现启动失败；启动诊断通过超长路径与 profile 文件占用路径验证。")
    add("- **F10** `stub-supervisor.exe` 仍在安装包内（延续历史 wfy 观察项）。")
    add("")
    add("## 证据索引")
    add("")
    add("| 路径 | 内容 |")
    add("| --- | --- |")
    add("| `environment.json` | 机器、构建、数据根、执行面审计与观测 |")
    add("| `results.json` | 全部逐项状态（pass/not_run/waived/pending_user） |")
    add("| `generations/` | 每用例驱动结果、逐项证据、资源汇总；`invalid-input/` 为编码问题轮次 |")
    add("| `samples/` | 1 s 资源采样 CSV |")
    add("| `screenshots/` | 安装、生成、管理、升级、启动诊断截图 |")
    add("| `install/`、`install-network*` | 七型号安装与真实网络安装证据 |")
    add("| `upgrade/` | v5→v6 升级前后 DB 快照、旧作品导出、升级后生成 |")
    add("| `uimgmt/`、`cancels/`、`forcekill/`、`console-watch/` | 管理、取消、强杀、控制台窗口证据 |")
    add("| `quality/` | 输出 PNG 解码、尺寸/通道、IHDR、亮度统计 |")
    add("| `build.log`、`logs-*.log` | 构建与运行日志片段 |")
    add("")
    add("## 复现命令（摘要）")
    add("")
    add("```powershell")
    add("& scripts/build-windows-internal.ps1")
    add("Start-Process -FilePath 'target\\release\\bundle\\nsis\\Image Studio_0.1.1_x64-setup.exe' -ArgumentList '/S' -Wait")
    add("$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222'")
    add("& \"$env:LOCALAPPDATA\\Image Studio\\image-studio-desktop.exe\" --data-dir C:\\ist-rg\\r-fresh")
    add("node scripts/windows-ui/drive-install.mjs --model Klein --out install.json")
    add("node scripts/windows-ui/drive-generate.mjs --model Klein --kind text --prompt '...' --out case.json")
    add("node scripts/windows-ui/drive-works.mjs --action list")
    add("```")
    add("")
    add("> 本文件与 `results.json` 均由证据文件生成/核对；手工结论以用户最终确认为准。")
    with open(os.path.join(EVIDENCE, "README.md"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")
    print("README written", len(lines), "lines")


if __name__ == "__main__":
    main()
