import { describe, expect, it } from 'vitest';
import { filterCatalog, type ModelEntry } from './model-catalog';

const base: ModelEntry = {
  id: 'z-image-turbo-mflux-v1', displayName: 'Z-Image Turbo', variant: 'Tongyi-MAI/Z-Image-Turbo',
  family: 'Z-Image', summary: '', description: '', capabilities: [{ id: 'text_to_image', label: '文生图' }],
  productStatus: 'implemented', runtime: 'MFLUX / MLX', license: 'Apache-2.0', source: '', revision: 'abc',
  modelBytes: 1, knownArtifactBytes: 1, requirements: [], exclusions: [], verifiedDevices: [],
  recipe: 'q4', installId: 'z-image-turbo-mflux-v1', installerSupported: true, installPhase: 'ready',
  componentsReady: true, runtimeReady: true, deviceVerified: true, executable: true, busy: false,
  receivedBytes: 100, installTotalBytes: 100, runtimeBytes: 20, requiredBytes: 300, error: null,
  canInstall: false, canContinue: false, canStop: false, canUse: true,
};
const planned: ModelEntry = {
  ...base, id: 'realesrgan-x4plus', displayName: 'Real-ESRGAN x4plus', variant: 'RealESRGAN_x4plus',
  family: 'Real-ESRGAN', runtime: 'ncnn / Vulkan', capabilities: [{ id: 'upscale', label: '放大' }], productStatus: 'planned',
  installId: null, installerSupported: false, installPhase: 'not_supported', componentsReady: false,
  runtimeReady: false, deviceVerified: false, executable: false, canUse: false,
};

describe('model catalog filtering', () => {
  it('combines search and primary filters without promoting planned models', () => {
    expect(filterCatalog([base, planned], 'esrgan', 'executable', 'all')).toEqual([]);
    expect(filterCatalog([base, planned], 'esrgan', 'planned', 'upscale').map(entry => entry.id)).toEqual(['realesrgan-x4plus']);
  });

  it('searches family, variant, id and runtime', () => {
    expect(filterCatalog([base, planned], 'mflux', 'all', 'all')).toHaveLength(1);
    expect(filterCatalog([base, planned], 'flux', 'all', 'image_edit')).toEqual([]);
  });
});
