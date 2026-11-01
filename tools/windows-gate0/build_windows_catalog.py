"""Project the existing seven logical models into the Windows product catalog."""

import json
import os
from pathlib import Path

root = Path(__file__).resolve().parents[2]
source = root / "resources/model-catalog/catalog.json"
output = root / "resources/windows-foundation/model-catalog.json"
document = json.loads(source.read_text(encoding="utf-8"))

entries = document["entries"]
z_image = entries[0]
if z_image["id"] != "z-image-turbo-mflux-v1":
    raise RuntimeError("unexpected Z-Image catalog order")
z_image.update(
    {
        "id": "z-image-turbo-diffusers-cuda-nf4-v1",
        "variant": "Tongyi-MAI/Z-Image-Turbo · Diffusers CUDA NF4 v1",
        "summary": "通过 Windows Gate 0 的目标机本地文生图模型，当前固定单张 1024²、9 步。",
        "description": "使用独立 Diffusers CUDA 后端，固定 NF4、BF16 计算、memory-efficient SDPA、BF16 VAE tiling 与组件 CPU 卸载；不继承 macOS MFLUX q4 配方。",
        "productStatus": "implemented",
        "runtime": "Diffusers 0.36.0 / PyTorch 2.10.0 / CUDA 12.6",
        "source": "https://www.modelscope.cn/models/Tongyi-MAI/Z-Image-Turbo",
        "revision": "f332072aa78be7aecdf3ee76d5c247082da564a6",
        "modelBytes": 32848305533,
        "knownArtifactBytes": 32848305533,
        "requirements": [
            "ModelScope master 传输内容逐文件匹配固定上游 SHA-256",
            "RTX 3080 Ti sm86 CUDA 与私有无系统 Python 依赖的运行时",
            "1024² 单张、9 步、guidance 0.0 与 512 token 实际上限",
        ],
        "exclusions": [
            "Intel UHD 770 或 CPU 生成回退",
            "macOS MFLUX q4 安装/Ready 事实",
            "非方图、多张、源图与 LoRA",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · driver 596.49 · sm86 · Windows 10 22H2"
        ],
        "recipe": "NF4 · 9 步 · guidance 0.0 · 1024 × 1024",
        "installId": "z-image-turbo-diffusers-cuda-nf4-v1",
        "installerSupported": True,
    }
)

for entry in entries[1:]:
    entry.update(
        {
            "productStatus": "planned",
            "summary": "当前 Windows 版本尚未接入；仅保留型号说明，不能安装或执行。",
            "runtime": "当前 Windows 版本未接入",
            "installId": None,
            "installerSupported": False,
            "verifiedDevices": [],
        }
    )

document["reviewedDate"] = "2026-11-01"
document["entries"] = entries
temporary = output.with_suffix(".json.pending")
temporary.write_text(
    json.dumps(document, ensure_ascii=False, indent=2) + "\n",
    encoding="utf-8",
    newline="\n",
)
os.replace(temporary, output)
print(output)
