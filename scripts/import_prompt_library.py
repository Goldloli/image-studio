#!/usr/bin/env python3
"""Build the checked-in prompt inspiration catalog from a fixed upstream snapshot."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any


UPSTREAM_REPOSITORY = "https://github.com/freestylefly/awesome-gpt-image-2"
UPSTREAM_COMMIT = "63e6176fa01be51c000108dacd72716d3e46784b"
CATALOG_SCHEMA_VERSION = 1
CATALOG_VERSION = "2026-10-31"
MAX_IMAGE_BYTES = 16 * 1024 * 1024
EXPECTED_CASE_COUNT = 541
EXPECTED_MISSING_IDS = [12, 169, 170]

CATEGORY_CONFIG = {
    "Posters & Typography": ("posters-typography", "海报与排版"),
    "Photography & Realism": ("photography-realism", "摄影与写实"),
    "UI & Interfaces": ("ui-interfaces", "UI 与界面"),
    "Illustration & Art": ("illustration-art", "插画与艺术"),
    "Charts & Infographics": ("charts-infographics", "图表与信息图"),
    "Products & E-commerce": ("products-ecommerce", "产品与电商"),
    "Characters & People": ("characters-people", "角色与人物"),
    "Brand & Logos": ("brand-logos", "品牌与标志"),
    "Scenes & Storytelling": ("scenes-storytelling", "场景与叙事"),
    "History & Classical Themes": ("history-classical", "历史与古典"),
    "Architecture & Spaces": ("architecture-spaces", "建筑与空间"),
    "Documents & Publishing": ("documents-publishing", "文档与出版"),
    "Other Use Cases": ("other-use-cases", "其他创意"),
}

STYLE_LABELS = {
    "3D": "3D",
    "Architecture": "建筑",
    "Brand": "品牌",
    "Character": "角色",
    "Characters": "角色",
    "Charts": "图表",
    "Classical": "古典",
    "Documents": "文档",
    "History": "历史",
    "Illustration": "插画",
    "Infographic": "信息图",
    "Other Use Cases": "其他",
    "Photography": "摄影",
    "Poster": "海报",
    "Product": "产品",
    "Products": "产品",
    "Realistic": "写实",
    "Scenes": "场景",
    "UI": "界面",
}

SCENE_LABELS = {
    "Commerce": "商业",
    "Creative": "创意",
    "Education": "教育",
    "Fashion": "时尚",
    "Food": "美食",
    "History": "历史",
    "Social": "社交",
    "Story": "叙事",
    "Tech": "科技",
    "Travel": "旅行",
}

REFERENCE_PATTERNS = [
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"\breference (?:photo|photograph|image|picture)\b",
        r"\buploaded (?:photo|photograph|image|picture)\b",
        r"\buploaded (?:face|subject|person|character|portrait)\b",
        r"\bupload (?:a|an|the|your|one) [^\n.!?]{1,80}",
        r"\b(?:provided|source|input|attached) (?:photo|photograph|image|picture)\b",
        r"\bturn (?:this|the|your|an?) (?:photo|photograph|image|picture) into\b",
        r"\bbased on (?:this|the|your|an?) (?:photo|photograph|image|picture)\b",
        r"\b(?:same appearance|same face|same person) as (?:the )?reference\b",
        r"\b(?:face|subject|person) (?:used )?(?:100% )?as (?:the )?reference\b",
        r"(?:上传|参考图|参考照片|原图|照片重绘)",
    )
]

EDIT_PATTERNS = [
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"\bimage edit(?:ing)?\b",
        r"\bedit (?:this|the|an?) (?:photo|image|picture)\b",
        r"\breplace (?:\w+ ){0,4}(?:in|from) (?:this|the|an?) (?:photo|image|picture)\b",
        r"\bremove (?:\w+ ){0,4}from (?:this|the|an?) (?:photo|image|picture)\b",
        r"\b(?:recreate|stylize|redraw|re-render|remix) (?:this|the|my) (?:photo|photograph|image|picture)\b",
        r"\bfrom the uploaded subject\b",
        r"\binpaint",
        r"\boutpaint",
        r"\bexpand (?:this|the|an?) (?:photo|image|picture)\b",
        r"(?:替换背景|去除背景|抠图|重绘)",
    )
]

REFERENCE_HELP = "需要参考图或上传照片；当前产品只填入文字，不会把示例图当作参考输入。"
EDIT_HELP = "需要图片编辑能力；当前产品只填入文字，不会切换或启用尚未接入的编辑能力。"
VARIABLE_HELP = "提示词包含可替换变量；使用前请先替换方括号或花括号中的内容。"


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def variables_present(prompt: str) -> bool:
    return bool(
        re.search(r"\[[A-Za-z][A-Za-z0-9 /_.-]{0,48}\]", prompt)
        or re.search(r"\{[^{}\n]{1,60}\}", prompt)
    )


def classify_usage(prompt: str) -> tuple[str, bool, list[str], str]:
    reference = any(pattern.search(prompt) for pattern in REFERENCE_PATTERNS)
    editing = any(pattern.search(prompt) for pattern in EDIT_PATTERNS)
    variables = variables_present(prompt)
    notes: list[str] = []
    if editing:
        input_kind = "image-edit"
        notes.append(EDIT_HELP)
    elif reference:
        input_kind = "reference-image"
        notes.append(REFERENCE_HELP)
    else:
        input_kind = "text-to-image"
    if variables:
        notes.append(VARIABLE_HELP)
    review_status = "reviewed_by_rules"
    return input_kind, variables, notes, review_status


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as source:
        return json.load(source)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as target:
        json.dump(value, target, ensure_ascii=False, indent=2)
        target.write("\n")


def copy_text_normalized(source: Path, target: Path) -> None:
    text = source.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n")
    target.write_text(text, encoding="utf-8")


def validate_source(document: dict[str, Any]) -> list[dict[str, Any]]:
    cases = document.get("cases")
    if not isinstance(cases, list):
        raise ValueError("上游 cases 必须是数组")
    if document.get("totalCases") != EXPECTED_CASE_COUNT or len(cases) != EXPECTED_CASE_COUNT:
        raise ValueError(
            f"案例总数不符：声明 {document.get('totalCases')!r}，实际 {len(cases)}"
        )
    ids = [case.get("id") for case in cases]
    if any(isinstance(case_id, bool) or not isinstance(case_id, int) for case_id in ids):
        raise ValueError("每个案例必须有整数 ID")
    if len(set(ids)) != len(ids):
        raise ValueError("案例 ID 不唯一")
    for case in cases:
        for field in ("title", "image", "imageAlt", "prompt", "category", "githubUrl"):
            if not isinstance(case.get(field), str) or not case[field]:
                raise ValueError(f"案例 {case.get('id')} 缺少字段 {field}")
        if case["category"] not in CATEGORY_CONFIG:
            raise ValueError(f"案例 {case['id']} 使用未知分类 {case['category']}")
        if not case["image"].startswith("/images/"):
            raise ValueError(f"案例 {case['id']} 图片路径格式无效")
        for field in ("styles", "scenes"):
            values = case.get(field)
            if not isinstance(values, list) or any(not isinstance(value, str) for value in values):
                raise ValueError(f"案例 {case['id']} 的 {field} 必须是字符串数组")
    return cases


def validate_corrections(document: Any) -> dict[int, dict[str, Any]]:
    if not isinstance(document, dict) or document.get("schemaVersion") != 1:
        raise ValueError("人工修正表版本或结构无效")
    corrections = document.get("cases")
    if not isinstance(corrections, dict):
        raise ValueError("人工修正表 cases 必须是对象")
    result: dict[int, dict[str, Any]] = {}
    for key, value in corrections.items():
        try:
            case_id = int(key)
        except ValueError as error:
            raise ValueError(f"人工修正案例 ID 无效：{key}") from error
        if not isinstance(value, dict):
            raise ValueError(f"案例 {case_id} 的人工修正必须是对象")
        unknown = set(value) - {"inputKind", "note", "reviewStatus"}
        if unknown:
            raise ValueError(f"案例 {case_id} 包含未知人工修正字段：{sorted(unknown)}")
        if value.get("inputKind") not in {None, "text-to-image", "image-edit", "reference-image", "unknown"}:
            raise ValueError(f"案例 {case_id} 的 inputKind 无效")
        if value.get("reviewStatus") not in {None, "reviewed", "needs-review", "unknown"}:
            raise ValueError(f"案例 {case_id} 的 reviewStatus 无效")
        result[case_id] = value
    return result


def image_metadata(path: Path) -> tuple[int, str, str]:
    value = path.read_bytes()
    if len(value) > MAX_IMAGE_BYTES:
        raise ValueError(f"{path.name} 超过 16 MiB")
    if value.startswith(b"\xff\xd8") and value.rstrip().endswith(b"\xff\xd9"):
        mime_type = "image/jpeg"
    elif value.startswith(b"\x89PNG\r\n\x1a\n") and value.endswith(
        b"\x00\x00\x00\x00IEND\xaeB`\x82"
    ):
        mime_type = "image/png"
    else:
        raise ValueError(f"{path.name} 不是完整 JPEG 或 PNG")
    return len(value), sha256_bytes(value), mime_type


def normalize_case(
    case: dict[str, Any],
    images: Path,
    corrections: dict[int, dict[str, Any]],
) -> dict[str, Any]:
    case_id = int(case["id"])
    filename = case["image"].rsplit("/", 1)[-1]
    image_path = images / filename
    if not image_path.is_file():
        raise ValueError(f"案例 {case_id} 缺少图片 {filename}")
    byte_count, image_hash, mime_type = image_metadata(image_path)
    category_id, category_label = CATEGORY_CONFIG[case["category"]]
    input_kind, variables, notes, review_status = classify_usage(case["prompt"])
    correction = corrections.get(case_id, {})
    if correction:
        input_kind = correction.get("inputKind", input_kind)
        review_status = correction.get("reviewStatus", "reviewed")
        if note := correction.get("note"):
            notes.append(str(note))
    unknown_styles = sorted(set(case["styles"]) - set(STYLE_LABELS))
    unknown_scenes = sorted(set(case["scenes"]) - set(SCENE_LABELS))
    if unknown_styles or unknown_scenes:
        raise ValueError(
            f"案例 {case_id} 包含未知风格/场景：{unknown_styles + unknown_scenes}"
        )
    source_url = case.get("sourceUrl")
    if source_url is not None and not isinstance(source_url, str):
        raise ValueError(f"案例 {case_id} 的 sourceUrl 类型无效")
    return {
        "id": f"awesome-gpt-image-2:case:{case_id}",
        "sourceCaseId": case_id,
        "title": case["title"],
        "prompt": case["prompt"],
        "promptSha256": sha256_bytes(case["prompt"].encode("utf-8")),
        "imageAlt": case["imageAlt"],
        "featured": bool(case.get("featured")),
        "category": {
            "id": category_id,
            "label": category_label,
            "sourceLabel": case["category"],
        },
        "styles": [
            {"sourceLabel": value, "label": STYLE_LABELS[value]}
            for value in case["styles"]
        ],
        "scenes": [
            {"sourceLabel": value, "label": SCENE_LABELS[value]}
            for value in case["scenes"]
        ],
        "image": {
            "path": case["image"].lstrip("/"),
            "filename": filename,
            "sha256": image_hash,
            "bytes": byte_count,
            "mimeType": mime_type,
        },
        "source": {
            "label": case.get("sourceLabel") or "未署名",
            "url": source_url or None,
            "githubUrl": (
                f"{UPSTREAM_REPOSITORY}/blob/{UPSTREAM_COMMIT}/"
                f"docs/gallery-part-2.md#case-{case_id}"
            ),
        },
        "usage": {
            "inputKind": input_kind,
            "hasVariables": variables,
            "notes": notes,
            "reviewStatus": review_status,
        },
        "model": {
            "upstreamProject": "awesome-gpt-image-2",
            "verification": "unverified",
            "exactModelId": None,
        },
        "rights": {
            "commercialUse": "unverified",
            "redistribution": "unverified",
            "sourceReferenceRequired": True,
        },
    }


def build_catalog(
    source_root: Path,
    images_root: Path,
    corrections_path: Path,
    output_root: Path,
) -> dict[str, Any]:
    source = load_json(source_root / "cases.json")
    cases = validate_source(source)
    corrections = validate_corrections(load_json(corrections_path))
    known_ids = {int(case["id"]) for case in cases}
    unknown_corrections = sorted(set(corrections) - known_ids)
    if unknown_corrections:
        raise ValueError(f"人工修正表引用未知案例：{unknown_corrections}")

    normalized = [
        normalize_case(case, images_root, corrections)
        for case in sorted(cases, key=lambda value: value["id"], reverse=True)
    ]
    category_counts = Counter(case["category"]["id"] for case in normalized)
    categories = [
        {
            "id": category_id,
            "label": label,
            "sourceLabel": source_label,
            "count": category_counts[category_id],
        }
        for source_label, (category_id, label) in CATEGORY_CONFIG.items()
    ]
    categories.sort(
        key=lambda value: (
            -value["count"],
            list(CATEGORY_CONFIG.values()).index(
                next(
                    item
                    for item in CATEGORY_CONFIG.values()
                    if item[0] == value["id"]
                )
            ),
        )
    )

    duplicate_image_names = [
        filename
        for filename, count in Counter(
            case["image"]["filename"] for case in normalized
        ).items()
        if count > 1
    ]
    if duplicate_image_names:
        raise ValueError(f"图片映射不唯一：{duplicate_image_names[:10]}")

    catalog = {
        "schemaVersion": CATALOG_SCHEMA_VERSION,
        "catalogVersion": CATALOG_VERSION,
        "upstreamRepository": UPSTREAM_REPOSITORY,
        "upstreamCommit": UPSTREAM_COMMIT,
        "importedAt": "2026-10-31",
        "sourceCount": source["totalCases"],
        "importedCount": len(normalized),
        "categories": categories,
        "styles": [
            {"sourceLabel": source_label, "label": label}
            for source_label, label in STYLE_LABELS.items()
        ],
        "scenes": [
            {"sourceLabel": source_label, "label": label}
            for source_label, label in SCENE_LABELS.items()
        ],
        "sourceRights": {
            "repositoryLicense": "MIT",
            "licensePath": "LICENSE",
            "disclaimerPath": "DISCLAIMER.md",
            "thirdPartyImages": "source-required",
            "commercialUse": "unverified",
            "redistribution": "unverified",
        },
        "imageCachePolicy": {
            "bundled": False,
            "maxFileBytes": MAX_IMAGE_BYTES,
            "maxCacheBytes": 268435456,
            "allowedMimeTypes": ["image/jpeg", "image/png"],
        },
        "cases": normalized,
    }

    missing_ids = [
        value
        for value in range(1, max(known_ids) + 1)
        if value not in known_ids
    ]
    report = {
        "schemaVersion": 1,
        "catalogVersion": CATALOG_VERSION,
        "upstreamCommit": UPSTREAM_COMMIT,
        "importedCount": len(normalized),
        "uniqueIdCount": len({case["id"] for case in normalized}),
        "uniqueImageCount": len(normalized),
        "categoryCount": len(categories),
        "categoryCounts": dict(category_counts),
        "featuredCount": sum(case["featured"] for case in normalized),
        "emptySourceUrlCount": sum(case["source"]["url"] is None for case in normalized),
        "emptySourceUrlCaseIds": [
            case["sourceCaseId"]
            for case in normalized
            if case["source"]["url"] is None
        ],
        "missingCaseIds": missing_ids,
        "expectedMissingCaseIds": EXPECTED_MISSING_IDS,
        "missingIdsMatch": sorted(missing_ids) == EXPECTED_MISSING_IDS,
        "imageBytesTotal": sum(case["image"]["bytes"] for case in normalized),
        "imageBytesMaximum": max(case["image"]["bytes"] for case in normalized),
        "usageCounts": dict(
            Counter(case["usage"]["inputKind"] for case in normalized)
        ),
        "variableCaseCount": sum(
            case["usage"]["hasVariables"] for case in normalized
        ),
        "manualCorrectionCount": len(corrections),
        "errors": [],
    }

    output_root.mkdir(parents=True, exist_ok=True)
    write_json(output_root / "catalog.json", catalog)
    write_json(output_root / "import-report.json", report)
    copy_text_normalized(source_root / "LICENSE", output_root / "LICENSE")
    copy_text_normalized(source_root / "disclaimer.md", output_root / "DISCLAIMER.md")
    return report


def validate_checked_in_catalog(path: Path) -> None:
    catalog = load_json(path)
    cases = catalog.get("cases")
    if catalog.get("schemaVersion") != CATALOG_SCHEMA_VERSION:
        raise ValueError("目录 schemaVersion 无效")
    if catalog.get("upstreamCommit") != UPSTREAM_COMMIT:
        raise ValueError("目录上游提交与导入器不一致")
    if not isinstance(cases, list) or len(cases) != EXPECTED_CASE_COUNT:
        raise ValueError("已提交目录案例数不是 541")
    ids = [case.get("id") for case in cases]
    if len(set(ids)) != len(ids):
        raise ValueError("已提交目录案例 ID 不唯一")
    categories = {case.get("category", {}).get("sourceLabel") for case in cases}
    if categories != set(CATEGORY_CONFIG):
        raise ValueError("已提交目录未覆盖全部 13 个分类")
    for case in cases:
        if case["image"]["bytes"] <= 0 or len(case["image"]["sha256"]) != 64:
            raise ValueError(f"案例 {case['sourceCaseId']} 图片元数据无效")
        if not case["prompt"] or case["prompt"] == case.get("promptPreview"):
            raise ValueError(f"案例 {case['sourceCaseId']} 提示词不是完整原文")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--images-root", type=Path, required=True)
    parser.add_argument("--corrections", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        validate_checked_in_catalog(args.output_root / "catalog.json")
        print("prompt library catalog is complete")
        return
    report = build_catalog(
        args.source_root,
        args.images_root,
        args.corrections,
        args.output_root,
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
