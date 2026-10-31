#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path

from playwright.sync_api import expect, sync_playwright


def open_library(page) -> None:
    page.get_by_label("主导航").get_by_role("button", name="灵感库").click()
    expect(page.get_by_role("heading", name="灵感库", exact=True)).to_be_visible()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:1420")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("verification/prompt-library-20261031"),
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    page_errors: list[str] = []
    console_errors: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        context = browser.new_context(viewport={"width": 1280, "height": 800})
        context.grant_permissions(["clipboard-read", "clipboard-write"], origin=args.url)
        page = context.new_page()
        page.on("pageerror", lambda error: page_errors.append(str(error)))
        page.on("console", lambda message: console_errors.append(message.text) if message.type == "error" else None)
        page.route("https://raw.githubusercontent.com/**", lambda route: route.abort())
        page.goto(args.url, wait_until="networkidle")
        page.evaluate("localStorage.clear()")
        page.reload(wait_until="networkidle")
        page.screenshot(path=str(args.output / "browser-initial.png"))
        if page.get_by_role("button", name="灵感库").count() == 0:
            raise RuntimeError(
                "应用未渲染灵感库入口"
                + f"\npage_errors={page_errors}"
                + f"\nconsole_errors={console_errors[-10:]}"
                + "\nbody="
                + page.locator("body").inner_text()[:2000]
            )

        open_library(page)
        expect(page.locator(".prompt-card")).to_have_count(541)
        page.screenshot(path=str(args.output / "browser-1280-library.png"))

        search = page.get_by_placeholder("搜索标题、完整提示词、分类或来源")
        search.fill("幼儿词汇拆解学习卡")
        expect(page.locator(".prompt-card")).to_have_count(1)
        page.locator(".prompt-card-main").click()
        detail = page.get_by_role("dialog", name="幼儿词汇拆解学习卡")
        expect(detail).to_be_visible()
        expect(detail.locator(".prompt-full-text")).to_contain_text("Feature [FRUIT]")
        full_prompt = detail.locator(".prompt-full-text").inner_text()
        detail.get_by_role("button", name="复制原文").click()
        expect(detail.get_by_role("button", name="已复制")).to_be_visible()
        assert page.evaluate("navigator.clipboard.readText()") == full_prompt
        detail.get_by_role("button", name="收藏", exact=True).click()
        expect(detail.get_by_role("button", name="已收藏")).to_be_visible()
        page.keyboard.press("Shift+Tab")
        assert page.evaluate(
            "Boolean(document.activeElement && document.activeElement.closest('.prompt-detail'))"
        )
        page.screenshot(path=str(args.output / "browser-1280-detail.png"))
        detail.get_by_role("button", name="用于当前会话").click()
        expect(page.get_by_label("图片提示词")).to_contain_text("Feature [FRUIT]")

        open_library(page)
        close_detail = page.get_by_role("button", name="关闭案例详情")
        if close_detail.count():
            close_detail.click()
        page.locator(".prompt-card-main").click()
        page.get_by_role("dialog", name="幼儿词汇拆解学习卡").get_by_role(
            "button", name="新建会话并填入"
        ).click()
        expect(page.locator(".conversation-heading h1")).to_have_text(
            "幼儿词汇拆解学习卡"
        )
        expect(page.get_by_label("图片提示词")).to_have_value(
            re.compile(r"Feature \[FRUIT\]")
        )

        composer = page.get_by_label("图片提示词")
        composer.fill("保留我的草稿")
        open_library(page)
        close_detail = page.get_by_role("button", name="关闭案例详情")
        if close_detail.count():
            close_detail.click()
        search.fill("旅行纪念珐琅徽章")
        expect(page.locator(".prompt-card")).to_have_count(1)
        page.locator(".prompt-card-main").click()
        page.get_by_role("dialog", name="旅行纪念珐琅徽章").get_by_role(
            "button", name="用于当前会话"
        ).click()
        replacement = page.get_by_role("dialog", name="替换为灵感提示词")
        expect(replacement).to_be_visible()
        replacement.get_by_role("button", name="替换").click()
        expect(page.get_by_label("图片提示词")).to_have_value(
            re.compile(r"travel souvenir enamel pin badge")
        )

        open_library(page)
        close_detail = page.get_by_role("button", name="关闭案例详情")
        if close_detail.count():
            close_detail.click()
        open_library(page)
        search.fill("")
        page.set_viewport_size({"width": 960, "height": 640})
        expect(page.get_by_role("heading", name="灵感库", exact=True)).to_be_visible()
        page.screenshot(path=str(args.output / "browser-960-library.png"), full_page=True)

        browser.close()

    if page_errors:
        raise RuntimeError("页面运行错误：" + "\n".join(page_errors))
    print(f"prompt library browser flow passed; screenshots={args.output}")


if __name__ == "__main__":
    main()
