"""Project the reviewed macOS catalog into the Windows product catalog.

Regeneration must preserve Windows parity: all seven logical models are implemented with their
frozen Windows model-variant / install / runtime / worker identities from design.md sections 10-11
(`openspec/changes/achieve-windows-model-parity`), plus hardware thresholds, quantization, license
notice and Windows execution status. Do not force entries 2-7 back to "planned"; that was the
pre-parity state and it silently reverts the change.

Usage:
    python tools/windows-gate0/build_windows_catalog.py           # regenerate the JSON
    python tools/windows-gate0/build_windows_catalog.py --check   # verify the JSON is current
"""

import argparse
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parents[2]
source = root / "resources/model-catalog/catalog.json"
output = root / "resources/windows-foundation/model-catalog.json"

REVIEWED_DATE = "2026-11-04"

# Windows model-variant ids are also the install ids (design.md section 11).
Z_IMAGE_ID = "z-image-turbo-diffusers-cuda-nf4-v1"
KLEIN_ID = "flux2-klein-4b-distilled-windows-v1"
BIREFNET_ID = "birefnet-general-epoch-244-windows-v1"
REALESRGAN_ID = "realesrgan-x4plus-windows-v1"
QWEN_ID = "qwen-image-2.1-windows-v1"
QWEN_TURBO_ID = "qwen-image-2.1-turbo-windows-v1"
MOSS_ID = "moss-image-sft-windows-v1"

V2_RUNTIME = "v2 共享运行时 · Python 3.12.10 / PyTorch 2.10.0 / CUDA 12.6 / diffusers 0.41"

# Fields appended after the fields shared with the macOS catalog. RegisteredModel keeps this order
# through serde(rename_all = "camelCase"); the Rust struct denies unknown fields, so the key set
# must stay in sync with apps/desktop/src/adapters/catalog.rs.
WINDOWS_FIELDS = (
    "minVramGb",
    "minRamGb",
    "quantization",
    "licenseNotice",
    "windowsStatus",
)

WINDOWS_OVERRIDES = {
    Z_IMAGE_ID: {
        "id": Z_IMAGE_ID,
        "variant": "Tongyi-MAI/Z-Image-Turbo · Diffusers CUDA NF4 v1",
        "summary": "通过 Windows Gate 0 的目标机本地文生图模型，当前固定单张 1024²、9 步。",
        "description": "使用独立 Diffusers CUDA 后端，固定 NF4、BF16 计算、memory-efficient SDPA、BF16 VAE tiling 与组件 CPU 卸载；不继承 macOS MFLUX q4 配方。",
        "productStatus": "implemented",
        "runtime": "Diffusers CUDA NF4 v1 · Python 3.12.10 / PyTorch 2.10.0 / CUDA 12.6",
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
        "installId": Z_IMAGE_ID,
        "installerSupported": True,
    },
    KLEIN_ID: {
        "id": KLEIN_ID,
        "variant": "black-forest-labs/FLUX.2-klein-4B · distilled",
        "summary": "Windows 版文生图与单图指令编辑模型，固定 4 步、guidance 1.0 与 1024²。",
        "description": "固定 distilled 4B 快照，Windows 使用 NF4 文本编码器与 NF4 transformer，并在独立 worker 进程中启用组件 CPU offload；单图编辑使用参考图条件分支，不包含多参考编辑。",
        "productStatus": "implemented",
        "runtime": V2_RUNTIME,
        "license": "Apache-2.0",
        "source": "https://huggingface.co/black-forest-labs/FLUX.2-klein-4B",
        "revision": "e7b7dc27f91deacad38e78976d1f2b499d76a294",
        "modelBytes": 15980131745,
        "knownArtifactBytes": 15980131745,
        "requirements": [
            "冻结快照逐文件 SHA-256（HF 声明与 ModelScope 传输内容一致）",
            "独立 worker 进程内 NF4 + 组件 CPU offload，禁止同进程叠加其他装载策略",
            "单图编辑输入、取消和资源回收实机验收",
        ],
        "exclusions": [
            "Klein Base / 9B / 9B KV",
            "多参考编辑",
            "把文生图参数伪造成图片编辑",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · Windows 10 22H2 · Gate 0 CUDA 9.21 GiB / commit 19.13 GiB"
        ],
        "recipe": "NF4 + 组件卸载 · 4 步 · guidance 1.0 · 1024 × 1024",
        "installId": KLEIN_ID,
        "installerSupported": True,
    },
    BIREFNET_ID: {
        "id": BIREFNET_ID,
        "variant": "BiRefNet-general-epoch_244.onnx",
        "summary": "Windows 版去背景模型，输出透明 RGBA PNG 与逐像素蒙版。",
        "description": "固定官方 general epoch_244 ONNX 权重，Windows 使用共享 v2 运行时的 ONNX Runtime CPUExecutionProvider、float32、1024² 预处理，并按源图规范化尺寸输出 RGBA 与 8-bit mask。",
        "productStatus": "implemented",
        "runtime": "v2 共享运行时 · ONNX Runtime 1.31.1 CPU",
        "license": "MIT",
        "source": "https://github.com/ZhengPeng7/BiRefNet/releases/tag/v1",
        "revision": "v1 / asset 186740004 / sha256:58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c",
        "modelBytes": 972666916,
        "knownArtifactBytes": 972666916,
        "requirements": [
            "核对后的 ONNX 图与 CPU 运行时",
            "透明 PNG 与 mask 同 Job 原子提交",
            "CPU 路线实机验收",
        ],
        "exclusions": [
            "Core ML / MPS 认证",
            "Portrait 或其他 checkpoint",
            "把文件元数据当作已完成哈希",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · Windows 10 22H2 · ONNX Runtime CPU · Gate 0 commit 10.14 GiB / 可用 RAM 15.52 GiB"
        ],
        "recipe": "1024² 网络预处理 · 原尺寸输出 · CPU（无 GPU 常驻）",
        "installId": BIREFNET_ID,
        "installerSupported": True,
    },
    REALESRGAN_ID: {
        "id": REALESRGAN_ID,
        "variant": "RealESRGAN_x4plus · ncnn Vulkan (Windows)",
        "summary": "Windows 版 4× 放大模型，使用官方 ncnn Vulkan 便携包与固定 x4plus 权重。",
        "description": "固定官方 v0.2.5.0 Windows ncnn 包、x4plus 权重、单 GPU、tile 128 与 1:1:1 线程；输出 PNG，不启用 TTA。",
        "productStatus": "implemented",
        "runtime": "ncnn Vulkan v0.2.0 · 官方 Windows 便携包",
        "license": "Real-ESRGAN BSD-3-Clause · ncnn app MIT",
        "source": "https://github.com/xinntao/Real-ESRGAN/releases/tag/v0.2.5.0",
        "revision": "v0.2.5.0 Windows ncnn package / x4plus bin sha256:713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf",
        "modelBytes": 33540549,
        "knownArtifactBytes": 45474481,
        "requirements": [
            "官方 Windows ncnn 便携包（45,474,481 B / sha256 abc02804…）与 x4plus 权重分别校验",
            "显式指定 x4plus、4×、tile 128 与单 GPU 0",
            "分块接缝、取消和强杀回收实机验收",
        ],
        "exclusions": [
            "动漫专用模型",
            "人脸修复",
            "任意倍率重采样",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti via Vulkan device 0 · Windows 10 22H2 · Gate 0 真实 4× 运行通过"
        ],
        "recipe": "ncnn Vulkan · x4plus · tile 128 · 固定 4×",
        "installId": REALESRGAN_ID,
        "installerSupported": True,
    },
    QWEN_ID: {
        "id": QWEN_ID,
        "variant": "Qwen/Qwen-Image-2.1 · Windows staged NF4",
        "summary": "Windows 版 Qwen-Image 2.1 文生图模型，采用分阶段 NF4 装载与安装期 prepared 量化缓存。",
        "description": "固定官方模型快照，安装期在 CPU 上把文本编码器与 transformer 量化为 NF4 并持久化；生成时按文本编码 → DiT 采样 → VAE 解码分阶段装载，只有一个大组件驻留 GPU。40 步 linear 采样、guidance 1.0、2048 token 预算。",
        "productStatus": "implemented",
        "runtime": V2_RUNTIME + " · staged NF4",
        "license": "qwen-research",
        "source": "https://huggingface.co/Qwen/Qwen-Image-2.1",
        "revision": "d26bb61231c349cf6b7896fa83353113880e1ba3",
        "modelBytes": 33131609424,
        "knownArtifactBytes": 33131609424,
        "requirements": [
            "ModelScope 官方源逐文件 revision 与 SHA-256 校验",
            "安装期 CPU NF4 prepare（可用内存预检 ≥28 GiB）与 prepared 缓存复用",
            "分阶段装载实测独占显存、进程 commit 与取消恢复",
        ],
        "exclusions": [
            "Qwen-Image 2.1 编辑与控制分支",
            "LoRA、低内存配方与小文本编码器",
            "非 1024² 画幅",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · Windows 10 22H2 · Gate 0 staged CUDA 6.00 GiB / commit 13.96 GiB"
        ],
        "recipe": "staged NF4 · 40 步 · guidance 1.0 · 1024 × 1024",
        "installId": QWEN_ID,
        "installerSupported": True,
    },
    QWEN_TURBO_ID: {
        "id": QWEN_TURBO_ID,
        "variant": "Qwen/Qwen-Image-2.1-Turbo · Windows staged NF4",
        "summary": "Windows 版 Qwen-Image 2.1 Turbo，官方 8 步采样曲线与分阶段 NF4 装载。",
        "description": "固定官方 Turbo revision 与 model_config，安装期在 CPU 上量化 prepared 缓存；生成时按文本编码 → DiT 采样（8 步官方 sample_sigmas）→ VAE 解码分阶段装载，只有一个大组件驻留 GPU。guidance 1.0、2048 token 预算。",
        "productStatus": "implemented",
        "runtime": V2_RUNTIME + " · staged NF4",
        "license": "qwen-research",
        "source": "https://huggingface.co/Qwen/Qwen-Image-2.1-Turbo",
        "revision": "fc13ba1986fcb905854dc8de2f52eee395f49b5b",
        "modelBytes": 32451607098,
        "knownArtifactBytes": 32451607098,
        "requirements": [
            "Turbo 权重与 model_config 独立核验，sample_sigmas 完整落盘",
            "安装期 CPU NF4 prepare（可用内存预检 ≥28 GiB）与 prepared 缓存复用",
            "独立取消、恢复和原生日常验收",
        ],
        "exclusions": [
            "仅把基础版步数改为 8 的伪 Turbo 路线",
            "LoRA、控制分支与非方图",
            "继承基础版实机认证",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · Windows 10 22H2 · Gate 0 staged CUDA 6.00 GiB / commit 13.95 GiB"
        ],
        "recipe": "staged NF4 · 8 步官方曲线 · 1024 × 1024",
        "installId": QWEN_TURBO_ID,
        "installerSupported": True,
    },
    MOSS_ID: {
        "id": MOSS_ID,
        "variant": "OpenMOSS-Team/MOSS-Image-SFT · Windows staged NF4",
        "summary": "Windows 版 MOSS-Image-SFT 文生图模型，采用分阶段 NF4 装载与安装期 prepared 量化缓存。",
        "description": "固定 MOSS-Image-SFT 快照并使用 worker 内置的已审阅 custom pipeline；安装期在 CPU 上把文本编码器与 DiT 量化为 NF4，生成时 50 步 moss_image 调度、guidance 3.0、空负提示词与负分支跳层 [22]，VAE 以 float32 分块解码。",
        "productStatus": "implemented",
        "runtime": V2_RUNTIME + " · staged NF4",
        "license": "Apache-2.0",
        "source": "https://huggingface.co/OpenMOSS-Team/MOSS-Image-SFT",
        "revision": "f972bd4b4b9e3852133cc5da954d87bc32cd8fb9",
        "modelBytes": 27219783878,
        "knownArtifactBytes": 27219783878,
        "requirements": [
            "ModelScope 官方源逐文件 revision 与 SHA-256 校验（含 custom pipeline 审阅）",
            "安装期 CPU NF4 prepare（可用内存预检 ≥28 GiB）与 prepared 缓存复用",
            "中文文字、JSON 布局、取消恢复和原生验收",
        ],
        "exclusions": [
            "MOSS-Image-Base",
            "社区 4/8 步蒸馏 LoRA",
            "可视化布局编辑器与像素级排版保证",
        ],
        "verifiedDevices": [
            "NVIDIA GeForce RTX 3080 Ti 12 GiB · Windows 10 22H2 · Gate 0 staged CUDA 5.84 GiB / commit 9.20 GiB"
        ],
        "recipe": "staged NF4 · 50 步 · CFG 3.0 · 1024 × 1024",
        "installId": MOSS_ID,
        "installerSupported": True,
    },
}

# Hardware thresholds from design.md section 10.2. The staged large models are only certified on
# the target machine (12 GB VRAM / 32 GB RAM, the install-time prepare needs the RAM); BiRefNet is
# a CPU route, Real-ESRGAN needs a Vulkan GPU but much less VRAM.
WINDOWS_METADATA = {
    Z_IMAGE_ID: {
        "minVramGb": 12,
        "minRamGb": 32,
        "quantization": "nf4",
        "licenseNotice": "Apache-2.0 · 可商用（保留版权与许可声明）",
        "windowsStatus": "已接入 · 独立 v1 运行时 NF4+组件卸载（Gate 0 实测 CUDA 8.87 GiB / commit 23.69 GiB）",
    },
    KLEIN_ID: {
        "minVramGb": 12,
        "minRamGb": 32,
        "quantization": "nf4",
        "licenseNotice": "Apache-2.0 · 可商用（保留版权与许可声明）",
        "windowsStatus": "已接入 · NF4+组件 CPU offload fresh-process（Gate 0 实测 CUDA 9.21 GiB / commit 19.13 GiB）",
    },
    BIREFNET_ID: {
        "minVramGb": 0,
        "minRamGb": 16,
        "quantization": "float32",
        "licenseNotice": "MIT",
        "windowsStatus": "已接入 · ONNX Runtime CPU 无 GPU 常驻（Gate 0 实测 commit 10.14 GiB / 可用 RAM 15.52 GiB）",
    },
    REALESRGAN_ID: {
        "minVramGb": 4,
        "minRamGb": 8,
        "quantization": "fp32",
        "licenseNotice": "BSD-3-Clause（Real-ESRGAN）/ MIT（ncnn 应用）",
        "windowsStatus": "已接入 · ncnn Vulkan 固定 4×/tile 128/单 GPU（Gate 0 真实 4× 运行通过）",
    },
    QWEN_ID: {
        "minVramGb": 12,
        "minRamGb": 32,
        "quantization": "nf4",
        "licenseNotice": "Qwen Research License：仅限非商业用途；商用需另行取得授权。",
        "windowsStatus": "已接入 · staged NF4 + prepared cache（Gate 0 实测 CUDA 6.00 GiB / commit 13.96 GiB）",
    },
    QWEN_TURBO_ID: {
        "minVramGb": 12,
        "minRamGb": 32,
        "quantization": "nf4",
        "licenseNotice": "Qwen Research License：仅限非商业用途；商用需另行取得授权。",
        "windowsStatus": "已接入 · staged NF4 + prepared cache（Gate 0 实测 CUDA 6.00 GiB / commit 13.95 GiB）",
    },
    MOSS_ID: {
        "minVramGb": 12,
        "minRamGb": 32,
        "quantization": "nf4",
        "licenseNotice": "Apache-2.0 · 可商用（保留版权与许可声明）",
        "windowsStatus": "已接入 · staged NF4 + prepared cache（Gate 0 实测 CUDA 5.84 GiB / commit 9.20 GiB）",
    },
}

SOURCE_ID_TO_WINDOWS_ID = {
    "z-image-turbo-mflux-v1": Z_IMAGE_ID,
    "flux2-klein-4b-distilled": KLEIN_ID,
    "birefnet-general-epoch-244": BIREFNET_ID,
    "realesrgan-x4plus": REALESRGAN_ID,
    "qwen-image-2.1-mflux-v1": QWEN_ID,
    "qwen-image-2.1-turbo-mflux-v1": QWEN_TURBO_ID,
    "moss-image-sft-mflux-v1": MOSS_ID,
}

# Logical capability ids projected per model: 8 (model, capability) pairs over 7 models.
EXPECTED_CAPABILITIES = {
    Z_IMAGE_ID: ["text_to_image"],
    KLEIN_ID: ["text_to_image", "image_edit"],
    BIREFNET_ID: ["background_removal"],
    REALESRGAN_ID: ["upscale"],
    QWEN_ID: ["text_to_image"],
    QWEN_TURBO_ID: ["text_to_image"],
    MOSS_ID: ["text_to_image"],
}


def build() -> dict:
    document = json.loads(source.read_text(encoding="utf-8"))
    entries = document["entries"]
    if len(entries) != 7:
        raise RuntimeError(f"expected the seven reviewed source models, found {len(entries)}")
    projected = []
    for entry in entries:
        source_id = entry["id"]
        windows_id = SOURCE_ID_TO_WINDOWS_ID.get(source_id)
        if windows_id is None:
            raise RuntimeError(f"unexpected reviewed model id: {source_id}")
        overlay = dict(WINDOWS_OVERRIDES[windows_id])
        overlay.update(WINDOWS_METADATA[windows_id])
        # Keep the macOS review narrative fields that are still true unless overridden above.
        entry.update(overlay)
        expected_keys = {
            "id",
            "displayName",
            "variant",
            "family",
            "summary",
            "description",
            "capabilities",
            "productStatus",
            "runtime",
            "license",
            "source",
            "revision",
            "modelBytes",
            "knownArtifactBytes",
            "requirements",
            "exclusions",
            "verifiedDevices",
            "recipe",
            "installId",
            "installerSupported",
            *WINDOWS_FIELDS,
        }
        missing = expected_keys - set(entry)
        if missing:
            raise RuntimeError(f"model {windows_id} is missing fields: {sorted(missing)}")
        for field in WINDOWS_FIELDS:
            if entry.get(field) is None:
                raise RuntimeError(f"model {windows_id} is missing Windows metadata {field}")
        if entry["installId"] != windows_id or entry["installerSupported"] is not True:
            raise RuntimeError(f"model {windows_id} lost its install identity")
        capability_ids = [capability["id"] for capability in entry["capabilities"]]
        if capability_ids != EXPECTED_CAPABILITIES[windows_id]:
            raise RuntimeError(
                f"model {windows_id} capabilities changed: {capability_ids}"
            )
        projected.append(entry)
    document["reviewedDate"] = REVIEWED_DATE
    document["entries"] = projected
    return document


def render(document: dict) -> str:
    return json.dumps(document, ensure_ascii=False, indent=2) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero when the committed Windows catalog is stale",
    )
    args = parser.parse_args()
    text = render(build())
    if args.check:
        current = output.read_text(encoding="utf-8")
        if current != text:
            raise SystemExit(f"{output} is stale; run tools/windows-gate0/build_windows_catalog.py")
        print(f"{output} is current")
        return
    temporary = output.with_suffix(".json.pending")
    temporary.write_text(text, encoding="utf-8", newline="\n")
    os.replace(temporary, output)
    print(output)


if __name__ == "__main__":
    main()
