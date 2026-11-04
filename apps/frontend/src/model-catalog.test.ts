import { describe, expect, it } from 'vitest';
import {
  canInstallModel,
  canStartGeneration,
  filterCatalog,
  hardwareBlocked,
  hardwareThresholdLabel,
  isNonCommercialLicense,
  licenseLabel,
  type ModelEntry,
} from './model-catalog';

const base: ModelEntry = {
  id: 'z-image-turbo-mflux-v1', displayName: 'Z-Image Turbo', variant: 'Tongyi-MAI/Z-Image-Turbo',
  family: 'Z-Image', summary: '', description: '', capabilities: [{ id: 'text_to_image', label: '文生图' }],
  productStatus: 'implemented', runtime: 'MFLUX / MLX', license: 'Apache-2.0', licenseNotice: 'Apache-2.0',
  source: '', revision: 'abc', modelBytes: 1, knownArtifactBytes: 1, requirements: [], exclusions: [],
  verifiedDevices: [], recipe: 'q4', installId: 'z-image-turbo-mflux-v1', installerSupported: true,
  minVramGb: null, minRamGb: null, quantization: null, windowsStatus: null,
  installPhase: 'ready', componentsReady: true, runtimeReady: true, deviceVerified: true, executable: true,
  hardwareEligible: true, hardwareReason: null,
  busy: false, receivedBytes: 100, installTotalBytes: 100, runtimeBytes: 20, requiredBytes: 300, error: null,
  canInstall: false, canContinue: false, canStop: false, canUse: true,
};
const queueOnly = (id: string, capability: string): ModelEntry => ({
  ...base, id, displayName: id, productStatus: 'planned', runtime: '规划中',
  capabilities: [{ id: capability, label: capability }],
  installId: null, installerSupported: false, installPhase: 'not_supported', componentsReady: false,
  runtimeReady: false, deviceVerified: false, executable: false, canUse: false, canInstall: false,
});

describe('model catalog filtering', () => {
  it('combines search and primary filters without promoting queue-only models', () => {
    expect(filterCatalog([base, queueOnly('realesrgan-x4plus', 'upscale')], 'esrgan', 'executable', 'all')).toEqual([]);
    expect(filterCatalog([base, queueOnly('realesrgan-x4plus', 'upscale')], 'esrgan', 'planned', 'upscale').map(entry => entry.id)).toEqual(['realesrgan-x4plus']);
  });

  it('searches family, variant, id and runtime', () => {
    expect(filterCatalog([base, queueOnly('realesrgan-x4plus', 'upscale')], 'mflux', 'all', 'all')).toHaveLength(1);
    expect(filterCatalog([base, queueOnly('flux2-klein-4b-distilled', 'image_edit')], 'flux', 'all', 'image_edit').map(entry => entry.id)).toEqual(['flux2-klein-4b-distilled']);
  });

  it('filters by device eligibility without hiding the reason', () => {
    const blocked: ModelEntry = {
      ...base, id: 'qwen-image-2.1-windows-v1', displayName: 'Qwen-Image 2.1',
      minVramGb: 12, minRamGb: 32, hardwareEligible: false,
      hardwareReason: '当前设备不满足硬件门槛：显存 ≥ 12 GB（当前 8 GB）。安装与生成入口已禁用。',
    };
    expect(filterCatalog([base, blocked], '', 'eligible', 'all').map(entry => entry.id)).toEqual([base.id]);
  });
});

describe('hardware gate projection', () => {
  const gated: ModelEntry = {
    ...base, id: 'flux2-klein-4b-distilled-windows-v1', displayName: 'FLUX.2 Klein 4B distilled',
    minVramGb: 12, minRamGb: 32, quantization: 'nf4',
    hardwareEligible: false,
    hardwareReason: '当前设备不满足硬件门槛：显存 ≥ 12 GB（当前 8 GB）、内存 ≥ 32 GB（当前 16 GB）。为避免安装或生成中途失败，安装与生成入口已禁用。',
    executable: false, canUse: false, canInstall: false,
  };

  it('disables install and generation when the device misses a threshold', () => {
    expect(hardwareBlocked(gated)).toBe(true);
    expect(canStartGeneration(gated)).toBe(false);
    expect(canInstallModel(gated)).toBe(false);
    expect(hardwareThresholdLabel(gated)).toBe('需要显存 ≥ 12 GB · 内存 ≥ 32 GB');
  });

  it('never re-enables a gated entry from a stale canUse flag', () => {
    const installable: ModelEntry = { ...base, canInstall: true, canUse: false, executable: false, installPhase: 'not_installed' };
    expect(hardwareBlocked(installable)).toBe(false);
    expect(canInstallModel(installable)).toBe(true);
    expect(hardwareBlocked(base)).toBe(false);
    expect(canStartGeneration(base)).toBe(true);
    // A payload that still claims canInstall/canUse must stay disabled on an ineligible device.
    const stale: ModelEntry = { ...gated, canInstall: true, canUse: true, executable: true };
    expect(canStartGeneration(stale)).toBe(false);
    expect(canInstallModel(stale)).toBe(false);
  });

  it('keeps the CPU route usable without a VRAM requirement', () => {
    const birefnet: ModelEntry = {
      ...base, id: 'birefnet-general-epoch-244-windows-v1', displayName: 'BiRefNet epoch_244',
      minVramGb: 0, minRamGb: 16, hardwareEligible: true,
    };
    expect(hardwareThresholdLabel(birefnet)).toBe('需要内存 ≥ 16 GB · CPU 路线（无显存门槛）');
    expect(canStartGeneration(birefnet)).toBe(true);
  });

  it('exposes the actionable reason for a blocked device', () => {
    expect(gated.hardwareReason).toContain('安装与生成入口已禁用');
  });
});

describe('license projection', () => {
  it('uses the registered license notice and flags the Qwen non-commercial license', () => {
    const qwen: ModelEntry = {
      ...base, id: 'qwen-image-2.1-windows-v1', license: 'qwen-research',
      licenseNotice: 'Qwen Research License：仅限非商业用途；商用需另行取得授权。',
    };
    expect(licenseLabel(qwen)).toContain('Qwen Research License');
    expect(isNonCommercialLicense(qwen)).toBe(true);
    expect(isNonCommercialLicense(base)).toBe(false);
  });
});
