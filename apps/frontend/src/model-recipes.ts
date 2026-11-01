export type TextRecipe = {
  modelId: string;
  capabilityId: string;
  displayName: string;
  shortName: string;
  steps: number;
  quantization: 4 | 8;
  aspects: readonly string[];
};

export const Z_IMAGE_MAC_MODEL_ID = 'z-image-turbo-mflux-v1';
export const Z_IMAGE_WINDOWS_MODEL_ID = 'z-image-turbo-diffusers-cuda-nf4-v1';
export const Z_IMAGE_MAC_CAPABILITY = 'image.text-to-image.z-image-turbo.v1';
export const Z_IMAGE_WINDOWS_CAPABILITY = 'image.text-to-image.z-image-turbo-diffusers-cuda-nf4.v1';
export const Z_IMAGE_MODEL_ID = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
  ? Z_IMAGE_WINDOWS_MODEL_ID
  : Z_IMAGE_MAC_MODEL_ID;
export const Z_IMAGE_CAPABILITY_FALLBACK = Z_IMAGE_MODEL_ID === Z_IMAGE_WINDOWS_MODEL_ID
  ? Z_IMAGE_WINDOWS_CAPABILITY
  : Z_IMAGE_MAC_CAPABILITY;
export const KLEIN_MODEL_ID = 'flux2-klein-4b-distilled';
export const QWEN_IMAGE_21_MODEL_ID = 'qwen-image-2.1-mflux-v1';
export const QWEN_IMAGE_21_TURBO_MODEL_ID = 'qwen-image-2.1-turbo-mflux-v1';
export const MOSS_IMAGE_SFT_MODEL_ID = 'moss-image-sft-mflux-v1';

export const QWEN_IMAGE_21_CAPABILITY = 'image.text-to-image.qwen-image-2.1.v1';
export const QWEN_IMAGE_21_TURBO_CAPABILITY = 'image.text-to-image.qwen-image-2.1-turbo.v1';
export const MOSS_IMAGE_SFT_CAPABILITY = 'image.text-to-image.moss-image-sft.v1';

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
  [KLEIN_MODEL_ID]: {
    modelId: KLEIN_MODEL_ID,
    capabilityId: 'image.text-to-image.flux2-klein-4b.v1',
    displayName: 'FLUX.2 Klein 4B',
    shortName: 'FLUX.2 Klein 4B',
    steps: 4,
    quantization: 4,
    aspects: ['1:1'],
  },
  [QWEN_IMAGE_21_MODEL_ID]: {
    modelId: QWEN_IMAGE_21_MODEL_ID,
    capabilityId: QWEN_IMAGE_21_CAPABILITY,
    displayName: 'Qwen-Image 2.1',
    shortName: 'Qwen-Image 2.1',
    steps: 40,
    quantization: 8,
    aspects: ['1:1'],
  },
  [QWEN_IMAGE_21_TURBO_MODEL_ID]: {
    modelId: QWEN_IMAGE_21_TURBO_MODEL_ID,
    capabilityId: QWEN_IMAGE_21_TURBO_CAPABILITY,
    displayName: 'Qwen-Image 2.1 Turbo',
    shortName: 'Qwen 2.1 Turbo',
    steps: 8,
    quantization: 8,
    aspects: ['1:1'],
  },
  [MOSS_IMAGE_SFT_MODEL_ID]: {
    modelId: MOSS_IMAGE_SFT_MODEL_ID,
    capabilityId: MOSS_IMAGE_SFT_CAPABILITY,
    displayName: 'MOSS-Image-SFT',
    shortName: 'MOSS-Image-SFT',
    steps: 50,
    quantization: 8,
    aspects: ['1:1'],
  },
};

export const textRecipeForModel = (modelId: string): TextRecipe | null => textRecipes[modelId] ?? null;
export const squareOnlyTextModel = (modelId: string): boolean => {
  const recipe = textRecipeForModel(modelId);
  return Boolean(recipe && !recipe.aspects.includes('4:3'));
};
