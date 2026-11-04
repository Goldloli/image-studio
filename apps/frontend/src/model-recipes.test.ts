import { describe, expect, it } from 'vitest';
import {
  MODEL_IDS,
  MODEL_PLATFORM,
  capabilityForModel,
  detectModelPlatform,
  modelDisplayName,
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
