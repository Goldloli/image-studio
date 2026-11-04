import { describe, expect, it } from 'vitest';
import {
  MODEL_IDS,
  MODEL_PLATFORM,
  capabilityForModel,
  crossPlatformModelAdvice,
  crossPlatformTextModelAdvice,
  crossPlatformToolModelAdvice,
  detectModelPlatform,
  isModelSupportedOn,
  modelDisplayName,
  platformModelIds,
  textRecipeForModel,
  windowsParityProjection,
} from './model-recipes';

const WINDOWS = MODEL_IDS.windows;
const MACOS = MODEL_IDS.macos;

// design.md section 11 frozen Windows model-variant ids (also the install ids).
const FROZEN_WINDOWS_MODEL_IDS = [
  'birefnet-general-epoch-244-windows-v1',
  'flux2-klein-4b-distilled-windows-v1',
  'moss-image-sft-windows-v1',
  'qwen-image-2.1-turbo-windows-v1',
  'qwen-image-2.1-windows-v1',
  'realesrgan-x4plus-windows-v1',
  'z-image-turbo-diffusers-cuda-nf4-v1',
].sort();

describe('platform model identity matrix', () => {
  it('detects Windows and macOS user agents', () => {
    expect(detectModelPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36')).toBe('windows');
    expect(detectModelPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15')).toBe('macos');
    expect(detectModelPlatform('Node.js/24.19.0')).toBe('macos');
  });

  it('registers exactly seven Windows model variants', () => {
    const models = new Set(windowsParityProjection().map(pair => pair.modelId));
    expect(models.size).toBe(7);
    expect([...models].sort()).toEqual(FROZEN_WINDOWS_MODEL_IDS);
  });

  it('projects eight Windows capability pairs, with Klein text and edit', () => {
    const pairs = windowsParityProjection();
    expect(pairs).toHaveLength(8);
    expect(pairs.filter(pair => pair.modelId === WINDOWS.klein.model).map(pair => pair.capabilityId))
      .toEqual([WINDOWS.klein.textCapability, WINDOWS.klein.editCapability]);
    expect(new Set(pairs.map(pair => `${pair.modelId}:${pair.capabilityId}`)).size).toBe(8);
  });

  it('uses the frozen Windows capability ids from design.md section 11', () => {
    expect(WINDOWS.zImage.capability).toBe('image.text-to-image.z-image-turbo-diffusers-cuda-nf4.v1');
    expect(WINDOWS.klein.textCapability).toBe('image.text-to-image.flux2-klein-4b.windows-v1');
    expect(WINDOWS.klein.editCapability).toBe('image.edit.flux2-klein-4b.windows-v1');
    expect(WINDOWS.birefnet.capability).toBe('image.remove-background.birefnet-general-epoch-244.windows-v1');
    expect(WINDOWS.realesrgan.capability).toBe('image.upscale.realesrgan-x4plus.windows-v1');
    expect(WINDOWS.qwen.capability).toBe('image.text-to-image.qwen-image-2.1.windows-v1');
    expect(WINDOWS.qwenTurbo.capability).toBe('image.text-to-image.qwen-image-2.1-turbo.windows-v1');
    expect(WINDOWS.moss.capability).toBe('image.text-to-image.moss-image-sft.windows-v1');
  });

  it('keeps the historical macOS ids untouched', () => {
    expect(MACOS.zImage.model).toBe('z-image-turbo-mflux-v1');
    expect(MACOS.klein.model).toBe('flux2-klein-4b-distilled');
    expect(MACOS.klein.editCapability).toBe('image.edit.flux2-klein-4b.v1');
    expect(MACOS.birefnet.model).toBe('birefnet-general-epoch-244');
    expect(MACOS.realesrgan.model).toBe('realesrgan-x4plus');
    expect(MACOS.qwen.model).toBe('qwen-image-2.1-mflux-v1');
  });

  it('provides frozen Windows text recipes', () => {
    expect(textRecipeForModel(WINDOWS.klein.model)?.steps).toBe(4);
    expect(textRecipeForModel(WINDOWS.klein.model)?.quantization).toBe(4);
    expect(textRecipeForModel(WINDOWS.qwen.model)?.steps).toBe(40);
    expect(textRecipeForModel(WINDOWS.qwen.model)?.quantization).toBe(4);
    expect(textRecipeForModel(WINDOWS.qwenTurbo.model)?.steps).toBe(8);
    expect(textRecipeForModel(WINDOWS.moss.model)?.steps).toBe(50);
    expect(textRecipeForModel(WINDOWS.zImage.model)?.steps).toBe(9);
    expect(textRecipeForModel(WINDOWS.zImage.model)?.aspects).toEqual(['1:1']);
  });

  it('maps the source-only tool capabilities on both platforms', () => {
    expect(capabilityForModel(WINDOWS.birefnet.model)).toBe(WINDOWS.birefnet.capability);
    expect(capabilityForModel(WINDOWS.realesrgan.model)).toBe(WINDOWS.realesrgan.capability);
    expect(capabilityForModel(WINDOWS.klein.model)).toBe(WINDOWS.klein.textCapability);
    expect(capabilityForModel(MACOS.birefnet.model)).toBe(MACOS.birefnet.capability);
  });

  it('labels known identities and preserves unknown ids', () => {
    expect(modelDisplayName(WINDOWS.klein.model)).toBe('FLUX.2 Klein 4B distilled');
    expect(modelDisplayName(MACOS.moss.model)).toBe('MOSS-Image-SFT');
    expect(modelDisplayName('removed-model')).toBe('removed-model');
  });

  it('resolves composer ids for the running platform', () => {
    expect(textRecipeForModel(MODEL_IDS[MODEL_PLATFORM].zImage.model)).not.toBeNull();
    expect(Object.keys(MODEL_IDS)).toEqual(['macos', 'windows']);
  });
});

describe('cross-platform text model selection', () => {
  const windowsTextIds = ['zImage', 'klein', 'qwen', 'qwenTurbo', 'moss'] as const;
  const macosTextIds = ['zImage', 'klein', 'qwen', 'qwenTurbo', 'moss'] as const;

  it('registers seven model ids per platform', () => {
    expect(platformModelIds('windows').size).toBe(7);
    expect(platformModelIds('macos').size).toBe(7);
  });

  it('keeps every Windows-native text model un-warned on Windows', () => {
    for (const key of windowsTextIds) {
      const modelId = MODEL_IDS.windows[key].model;
      expect(isModelSupportedOn('windows', modelId)).toBe(true);
      expect(crossPlatformTextModelAdvice('windows', modelId)).toBeNull();
    }
    expect(isModelSupportedOn('windows', MODEL_IDS.windows.birefnet.model)).toBe(true);
    expect(isModelSupportedOn('windows', MODEL_IDS.windows.realesrgan.model)).toBe(true);
  });

  it('warns on Windows for legacy macOS text models with the Windows target family', () => {
    for (const key of macosTextIds) {
      const modelId = MODEL_IDS.macos[key].model;
      expect(isModelSupportedOn('windows', modelId)).toBe(false);
      const advice = crossPlatformTextModelAdvice('windows', modelId);
      expect(advice).not.toBeNull();
      expect(advice).toContain('Windows 型号');
      expect(advice).toContain('Z-Image / Klein / Qwen / MOSS');
      expect(advice).not.toContain('请选择 Windows Z-Image');
    }
  });

  it('does not warn for unknown or tool model ids', () => {
    expect(crossPlatformTextModelAdvice('windows', 'removed-model')).toBeNull();
    expect(crossPlatformTextModelAdvice('windows', MODEL_IDS.macos.birefnet.model)).toBeNull();
    expect(crossPlatformTextModelAdvice('macos', MODEL_IDS.windows.realesrgan.model)).toBeNull();
  });

  it('keeps macOS-native text models un-warned and Windows ids warned on macOS', () => {
    for (const key of macosTextIds) {
      expect(crossPlatformTextModelAdvice('macos', MODEL_IDS.macos[key].model)).toBeNull();
    }
    for (const key of windowsTextIds) {
      const advice = crossPlatformTextModelAdvice('macos', MODEL_IDS.windows[key].model);
      expect(advice).not.toBeNull();
      expect(advice).toContain('Windows 端');
      expect(advice).toContain('macOS 型号');
    }
  });

  it('covers the source-only tool models in both directions', () => {
    const windowsTool = crossPlatformToolModelAdvice('windows', MACOS.birefnet.model);
    expect(windowsTool).not.toBeNull();
    expect(windowsTool).toContain('Windows BiRefNet 去背景');
    expect(windowsTool).toContain('核对源图');
    expect(crossPlatformToolModelAdvice('windows', MACOS.realesrgan.model)).toContain(
      'Windows Real-ESRGAN 放大',
    );
    expect(crossPlatformToolModelAdvice('windows', WINDOWS.birefnet.model)).toBeNull();
    expect(crossPlatformToolModelAdvice('windows', WINDOWS.realesrgan.model)).toBeNull();

    const macTool = crossPlatformToolModelAdvice('macos', WINDOWS.realesrgan.model);
    expect(macTool).not.toBeNull();
    expect(macTool).toContain('Windows 端');
    expect(macTool).toContain('macOS Real-ESRGAN 放大');
    expect(crossPlatformToolModelAdvice('macos', MACOS.birefnet.model)).toBeNull();

    // The combined helper keeps the text behaviour and adds the tool mirror.
    expect(crossPlatformModelAdvice('windows', MACOS.zImage.model)).toContain(
      'Z-Image / Klein / Qwen / MOSS',
    );
    expect(crossPlatformModelAdvice('windows', MACOS.birefnet.model)).toContain(
      'Windows BiRefNet 去背景',
    );
    expect(crossPlatformModelAdvice('windows', WINDOWS.qwenTurbo.model)).toBeNull();
    expect(crossPlatformModelAdvice('windows', 'removed-model')).toBeNull();
  });
});
