"""All ML imports happen after protocol stdout isolation in __main__."""
import hashlib
import os
from pathlib import Path


class Cancelled(Exception):
    pass


def checkpoint(cancel):
    if cancel.is_set():
        raise Cancelled()


def token_budget(tokenizer, prompt):
    """Mirror fixed LanguageTokenizer formatting, but explicitly disable truncation."""
    raw = tokenizer.tokenizer
    formatted = prompt
    if tokenizer.template:
        formatted = tokenizer.template.format(prompt)
    elif tokenizer.use_chat_template:
        formatted = raw.apply_chat_template(
            [{'role': 'user', 'content': prompt}], tokenize=False,
            add_generation_prompt=True, **tokenizer.chat_template_kwargs)
    ids = raw(formatted, padding=False, truncation=False,
              add_special_tokens=tokenizer.add_special_tokens)['input_ids']
    if len(ids) > 512:
        raise ValueError(f'提示词含模板共 {len(ids)} token，超过 512 token 上限；请缩短提示词。')
    return len(ids)


def generate(request, progress, cancel):
    checkpoint(cancel)
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader
    from mflux.models.z_image.weights.z_image_weight_definition import ZImageWeightDefinition
    from mflux.models.z_image.variants.z_image import ZImage
    progress('loading')
    tokenizers = TokenizerLoader.load_all(ZImageWeightDefinition.get_tokenizers(), request['model_path'])
    token_budget(tokenizers['z_image'], request['prompt'])
    checkpoint(cancel)

    class InstrumentedZImage(ZImage):
        def _encode_prompts(self, **kwargs):
            checkpoint(cancel)
            progress('encoding')
            return super()._encode_prompts(**kwargs)

        def _decode_latents(self, **kwargs):
            checkpoint(cancel)
            progress('decoding')
            return super()._decode_latents(**kwargs)

    class Steps:
        def call_before_loop(self, **kwargs):
            checkpoint(cancel)
            progress('sampling', 0, 9)

        def call_in_loop(self, t, **kwargs):
            checkpoint(cancel)
            # Upstream calls this before mx.eval; arrival is not GPU completion.
            progress('sampling', int(t) + 1, 9)

        def call_after_loop(self, **kwargs):
            checkpoint(cancel)

    model = InstrumentedZImage(quantize=4, model_path=request['model_path'])
    checkpoint(cancel)
    model.callbacks.register(Steps())
    image = model.generate_image(prompt=request['prompt'], seed=request['seed'],
                                 width=request['width'], height=request['height'],
                                 num_inference_steps=9, guidance=0.0, scheduler='linear')
    checkpoint(cancel)
    progress('writing')
    output = Path(request['output_dir']) / 'result.png'
    if output.exists() or output.is_symlink():
        raise ValueError('Attempt 输出路径已存在，拒绝覆盖。')
    image.save(path=str(output))
    checkpoint(cancel)
    size = output.stat().st_size
    if not 0 < size <= 32 * 1024 * 1024:
        raise ValueError('PNG 候选超过 32 MiB 上限。')
    with output.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        os.fsync(stream.fileno())
    return dict(relative_path='result.png', width=request['width'], height=request['height'],
                size_bytes=size, sha256=digest)
