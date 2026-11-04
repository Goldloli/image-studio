/**
 * Frozen model/recipe identities per platform.
 *
 * The macOS and Windows hosts use independent model-variant and capability ids (the macOS ids are
 * historical and must not be rewritten). The webview assets are shared, so the composer resolves
 * the ids from the running platform exactly like the existing Z-Image switch. The Windows ids below
 * are design.md section 11 / `studio_core::generation` frozen strings; keep them character-exact.
 */

export type ModelPlatform = 'windows' | 'macos';

export type PlatformModelIds = {
  zImage: { model: string; capability: string };
  klein: { model: string; textCapability: string; editCapability: string };
  birefnet: { model: string; capability: string };
  realesrgan: { model: string; capability: string };
  qwen: { model: string; capability: string };
  qwenTurbo: { model: string; capability: string };
  moss: { model: string; capability: string };
};

export const MODEL_IDS: Record<ModelPlatform, PlatformModelIds> = {
  macos: {
    zImage: {
      model: 'z-image-turbo-mflux-v1',
      capability: 'image.text-to-image.z-image-turbo.v1',
    },
    klein: {
      model: 'flux2-klein-4b-distilled',
      textCapability: 'image.text-to-image.flux2-klein-4b.v1',
      editCapability: 'image.edit.flux2-klein-4b.v1',
    },
    birefnet: {
      model: 'birefnet-general-epoch-244',
      capability: 'image.remove-background.birefnet-general-epoch-244.v1',
    },
    realesrgan: {
      model: 'realesrgan-x4plus',
      capability: 'image.upscale.realesrgan-x4plus.v1',
    },
    qwen: {
      model: 'qwen-image-2.1-mflux-v1',
      capability: 'image.text-to-image.qwen-image-2.1.v1',
    },
    qwenTurbo: {
      model: 'qwen-image-2.1-turbo-mflux-v1',
      capability: 'image.text-to-image.qwen-image-2.1-turbo.v1',
    },
    moss: {
      model: 'moss-image-sft-mflux-v1',
      capability: 'image.text-to-image.moss-image-sft.v1',
    },
  },
  windows: {
    zImage: {
      model: 'z-image-turbo-diffusers-cuda-nf4-v1',
      capability: 'image.text-to-image.z-image-turbo-diffusers-cuda-nf4.v1',
    },
    klein: {
      model: 'flux2-klein-4b-distilled-windows-v1',
      textCapability: 'image.text-to-image.flux2-klein-4b.windows-v1',
      editCapability: 'image.edit.flux2-klein-4b.windows-v1',
    },
    birefnet: {
      model: 'birefnet-general-epoch-244-windows-v1',
      capability: 'image.remove-background.birefnet-general-epoch-244.windows-v1',
    },
    realesrgan: {
      model: 'realesrgan-x4plus-windows-v1',
      capability: 'image.upscale.realesrgan-x4plus.windows-v1',
    },
    qwen: {
      model: 'qwen-image-2.1-windows-v1',
      capability: 'image.text-to-image.qwen-image-2.1.windows-v1',
    },
    qwenTurbo: {
      model: 'qwen-image-2.1-turbo-windows-v1',
      capability: 'image.text-to-image.qwen-image-2.1-turbo.windows-v1',
    },
    moss: {
      model: 'moss-image-sft-windows-v1',
      capability: 'image.text-to-image.moss-image-sft.windows-v1',
    },
  },
};

/** Pure platform detection so the matrix stays testable in a plain Node process. */
export const detectModelPlatform = (userAgent?: string): ModelPlatform => {
  const agent = userAgent ?? (typeof navigator !== 'undefined' ? navigator.userAgent : '');
  return /Windows/i.test(agent) ? 'windows' : 'macos';
};

export const MODEL_PLATFORM: ModelPlatform = detectModelPlatform();
const ids = MODEL_IDS[MODEL_PLATFORM];

export const Z_IMAGE_MAC_MODEL_ID = MODEL_IDS.macos.zImage.model;
export const Z_IMAGE_WINDOWS_MODEL_ID = MODEL_IDS.windows.zImage.model;
export const Z_IMAGE_MAC_CAPABILITY = MODEL_IDS.macos.zImage.capability;
export const Z_IMAGE_WINDOWS_CAPABILITY = MODEL_IDS.windows.zImage.capability;
export const Z_IMAGE_MODEL_ID = ids.zImage.model;
export const Z_IMAGE_CAPABILITY_FALLBACK = ids.zImage.capability;
export const KLEIN_MODEL_ID = ids.klein.model;
export const KLEIN_TEXT_CAPABILITY = ids.klein.textCapability;
export const KLEIN_EDIT_CAPABILITY = ids.klein.editCapability;
export const BIREFNET_MODEL_ID = ids.birefnet.model;
export const BIREFNET_CAPABILITY = ids.birefnet.capability;
export const REALESRGAN_MODEL_ID = ids.realesrgan.model;
export const REALESRGAN_CAPABILITY = ids.realesrgan.capability;
export const QWEN_IMAGE_21_MODEL_ID = ids.qwen.model;
export const QWEN_IMAGE_21_CAPABILITY = ids.qwen.capability;
export const QWEN_IMAGE_21_TURBO_MODEL_ID = ids.qwenTurbo.model;
export const QWEN_IMAGE_21_TURBO_CAPABILITY = ids.qwenTurbo.capability;
export const MOSS_IMAGE_SFT_MODEL_ID = ids.moss.model;
export const MOSS_IMAGE_SFT_CAPABILITY = ids.moss.capability;

export type TextRecipe = {
  modelId: string;
  capabilityId: string;
  displayName: string;
  shortName: string;
  steps: number;
  quantization: 4 | 8;
  aspects: readonly string[];
};

const textRecipes: Record<string, TextRecipe> = {
  [Z_IMAGE_MAC_MODEL_ID]: {
    modelId: Z_IMAGE_MAC_MODEL_ID,
    capabilityId: Z_IMAGE_MAC_CAPABILITY,
    displayName: 'Z-Image Turbo',
    shortName: 'Z-Image Turbo',
    steps: 9,
    quantization: 4,
    aspects: ['1:1', '4:3', '3:4'],
  },
  [Z_IMAGE_WINDOWS_MODEL_ID]: {
    modelId: Z_IMAGE_WINDOWS_MODEL_ID,
    capabilityId: Z_IMAGE_WINDOWS_CAPABILITY,
    displayName: 'Z-Image Turbo',
    shortName: 'Z-Image Turbo',
    steps: 9,
    quantization: 4,
    aspects: ['1:1'],
  },
  [MODEL_IDS.macos.klein.model]: {
    modelId: MODEL_IDS.macos.klein.model,
    capabilityId: MODEL_IDS.macos.klein.textCapability,
    displayName: 'FLUX.2 Klein 4B',
    shortName: 'FLUX.2 Klein 4B',
    steps: 4,
    quantization: 4,
    aspects: ['1:1'],
  },
  [MODEL_IDS.windows.klein.model]: {
    modelId: MODEL_IDS.windows.klein.model,
    capabilityId: MODEL_IDS.windows.klein.textCapability,
    displayName: 'FLUX.2 Klein 4B',
    shortName: 'FLUX.2 Klein 4B',
    steps: 4,
    quantization: 4,
    aspects: ['1:1'],
  },
  [MODEL_IDS.macos.qwen.model]: {
    modelId: MODEL_IDS.macos.qwen.model,
    capabilityId: MODEL_IDS.macos.qwen.capability,
    displayName: 'Qwen-Image 2.1',
    shortName: 'Qwen-Image 2.1',
    steps: 40,
    quantization: 8,
    aspects: ['1:1'],
  },
  [MODEL_IDS.windows.qwen.model]: {
    modelId: MODEL_IDS.windows.qwen.model,
    capabilityId: MODEL_IDS.windows.qwen.capability,
    displayName: 'Qwen-Image 2.1',
    shortName: 'Qwen-Image 2.1',
    steps: 40,
    quantization: 4,
    aspects: ['1:1'],
  },
  [MODEL_IDS.macos.qwenTurbo.model]: {
    modelId: MODEL_IDS.macos.qwenTurbo.model,
    capabilityId: MODEL_IDS.macos.qwenTurbo.capability,
    displayName: 'Qwen-Image 2.1 Turbo',
    shortName: 'Qwen 2.1 Turbo',
    steps: 8,
    quantization: 8,
    aspects: ['1:1'],
  },
  [MODEL_IDS.windows.qwenTurbo.model]: {
    modelId: MODEL_IDS.windows.qwenTurbo.model,
    capabilityId: MODEL_IDS.windows.qwenTurbo.capability,
    displayName: 'Qwen-Image 2.1 Turbo',
    shortName: 'Qwen 2.1 Turbo',
    steps: 8,
    quantization: 4,
    aspects: ['1:1'],
  },
  [MODEL_IDS.macos.moss.model]: {
    modelId: MODEL_IDS.macos.moss.model,
    capabilityId: MODEL_IDS.macos.moss.capability,
    displayName: 'MOSS-Image-SFT',
    shortName: 'MOSS-Image-SFT',
    steps: 50,
    quantization: 8,
    aspects: ['1:1'],
  },
  [MODEL_IDS.windows.moss.model]: {
    modelId: MODEL_IDS.windows.moss.model,
    capabilityId: MODEL_IDS.windows.moss.capability,
    displayName: 'MOSS-Image-SFT',
    shortName: 'MOSS-Image-SFT',
    steps: 50,
    quantization: 4,
    aspects: ['1:1'],
  },
};

export const textRecipeForModel = (modelId: string): TextRecipe | null => textRecipes[modelId] ?? null;
export const squareOnlyTextModel = (modelId: string): boolean => {
  const recipe = textRecipeForModel(modelId);
  return Boolean(recipe && !recipe.aspects.includes('4:3'));
};

/** The capability a text recipe/model selects, falling back to the tool models. */
export const capabilityForModel = (modelId: string): string =>
  textRecipeForModel(modelId)?.capabilityId
  ?? (modelId === MODEL_IDS.macos.birefnet.model || modelId === MODEL_IDS.windows.birefnet.model
    ? MODEL_IDS.macos.birefnet.model === modelId
      ? MODEL_IDS.macos.birefnet.capability
      : MODEL_IDS.windows.birefnet.capability
    : modelId === MODEL_IDS.macos.realesrgan.model || modelId === MODEL_IDS.windows.realesrgan.model
      ? MODEL_IDS.macos.realesrgan.model === modelId
        ? MODEL_IDS.macos.realesrgan.capability
        : MODEL_IDS.windows.realesrgan.capability
      : Z_IMAGE_CAPABILITY_FALLBACK);

const displayNames: Record<string, string> = {
  [Z_IMAGE_MAC_MODEL_ID]: 'Z-Image Turbo',
  [Z_IMAGE_WINDOWS_MODEL_ID]: 'Z-Image Turbo',
  [MODEL_IDS.macos.klein.model]: 'FLUX.2 Klein 4B distilled',
  [MODEL_IDS.windows.klein.model]: 'FLUX.2 Klein 4B distilled',
  [MODEL_IDS.macos.birefnet.model]: 'BiRefNet epoch_244 · 去背景',
  [MODEL_IDS.windows.birefnet.model]: 'BiRefNet epoch_244 · 去背景',
  [MODEL_IDS.macos.realesrgan.model]: 'Real-ESRGAN x4plus · 4× 放大',
  [MODEL_IDS.windows.realesrgan.model]: 'Real-ESRGAN x4plus · 4× 放大',
  [MODEL_IDS.macos.qwen.model]: 'Qwen-Image 2.1',
  [MODEL_IDS.windows.qwen.model]: 'Qwen-Image 2.1',
  [MODEL_IDS.macos.qwenTurbo.model]: 'Qwen-Image 2.1 Turbo',
  [MODEL_IDS.windows.qwenTurbo.model]: 'Qwen-Image 2.1 Turbo',
  [MODEL_IDS.macos.moss.model]: 'MOSS-Image-SFT',
  [MODEL_IDS.windows.moss.model]: 'MOSS-Image-SFT',
};

export const modelDisplayName = (modelId: string): string => displayNames[modelId] ?? modelId;

/** Every model-variant id that the given platform can execute (seven per platform). */
const PLATFORM_MODEL_IDS: Record<ModelPlatform, ReadonlySet<string>> = {
  macos: new Set(Object.values(MODEL_IDS.macos).map(entry => entry.model)),
  windows: new Set(Object.values(MODEL_IDS.windows).map(entry => entry.model)),
};

export const platformModelIds = (platform: ModelPlatform): ReadonlySet<string> =>
  PLATFORM_MODEL_IDS[platform];

export const isModelSupportedOn = (platform: ModelPlatform, modelId: string): boolean =>
  platformModelIds(platform).has(modelId);

const CROSS_PLATFORM_TEXT_ADVICE: Record<ModelPlatform, string> = {
  windows:
    '此会话保留来自其他平台的模型选择。请从模型菜单明确选择对应的 Windows 型号（Z-Image / Klein / Qwen / MOSS），并核对画幅与参数后再发送。',
  macos: '此会话保留 Windows 端的模型选择。请从模型菜单明确选择对应的 macOS 型号，并核对画幅后再发送。',
};

/**
 * Advisory text when a text recipe was frozen by the other platform.
 *
 * A text model that exists on the running platform is supported even when it is
 * not installed yet; only the other platform's model ids need a reselect hint.
 * Returns null for tool recipes and for models of the running platform.
 */
export const crossPlatformTextModelAdvice = (
  platform: ModelPlatform,
  modelId: string,
): string | null => {
  if (!textRecipeForModel(modelId)) return null;
  if (isModelSupportedOn(platform, modelId)) return null;
  return CROSS_PLATFORM_TEXT_ADVICE[platform];
};

/** Every Windows parity (model, capability) pair: 7 models, 8 pairs (Klein has two). */
export const windowsParityProjection = (): { modelId: string; capabilityId: string }[] => {
  const windows = MODEL_IDS.windows;
  return [
    { modelId: windows.zImage.model, capabilityId: windows.zImage.capability },
    { modelId: windows.klein.model, capabilityId: windows.klein.textCapability },
    { modelId: windows.klein.model, capabilityId: windows.klein.editCapability },
    { modelId: windows.birefnet.model, capabilityId: windows.birefnet.capability },
    { modelId: windows.realesrgan.model, capabilityId: windows.realesrgan.capability },
    { modelId: windows.qwen.model, capabilityId: windows.qwen.capability },
    { modelId: windows.qwenTurbo.model, capabilityId: windows.qwenTurbo.capability },
    { modelId: windows.moss.model, capabilityId: windows.moss.capability },
  ];
};
