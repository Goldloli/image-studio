"""Compatibility shim: run the MOSS-Image custom DiT with bitsandbytes weights.

The upstream ``pipeline_moss_image.py`` derives its compute dtype from
``<module>.weight.dtype``. That is correct for bf16 weights, but a
bitsandbytes linear reports its *storage* dtype there (``int8`` for 8-bit,
``uint8`` for NF4), so the custom forward would cast activations to integers and
fail in ``silu``.

This shim replaces the three forwards that inspect ``weight.dtype`` with the
same math using an explicit bf16 compute dtype. Everything else (RMSNorm,
attention, modulation, rope, scheduler, pipeline loop) is left untouched. The
shim only exists for the Windows staged experiments; the product worker vendors
the fixed upstream module plus this documented compatibility patch.
"""

from __future__ import annotations

import torch
from diffusers.models.modeling_outputs import Transformer2DModelOutput

COMPUTE_DTYPE = torch.bfloat16


def install(module) -> None:
    def block_forward(self, x, rope, modulation):
        shift_a, scale_a, gate_a, shift_f, scale_f, gate_f = modulation.unbind(1)
        y = self.norm1(x) * (1 + scale_a[:, None]) + shift_a[:, None]
        y = y.to(COMPUTE_DTYPE)
        q = self.q_norm(self.proj["q"](y).unflatten(-1, (self.heads, -1))).flatten(2)
        k = self.k_norm(self.proj["k"](y).unflatten(-1, (self.heads, -1))).flatten(2)
        x = x + self.proj["o"](
            module._attention(q, k, self.proj["v"](y), rope, self.heads)
        ) * gate_a[:, None]
        y = self.norm2(x) * (1 + scale_f[:, None]) + shift_f[:, None]
        hidden, gate = self.proj["ffn_up_gate"](y.to(COMPUTE_DTYPE)).chunk(2, -1)
        return x + self.proj["ffn_down"](module._situ(hidden, gate)) * gate_f[:, None]

    def text_projection_forward(self, x, rope):
        x = self.proj(x.to(COMPUTE_DTYPE))
        for block in self.blocks:
            x = block(x, rope)
        return x

    def transformer_forward(
        self,
        hidden_states,
        timestep,
        encoder_hidden_states,
        encoder_attention_mask=None,
        return_dict=True,
        skip_layers=None,
    ):
        skip_layers = module._validate_skip_layers(skip_layers, len(self.layers))
        if hidden_states.ndim != 4 or hidden_states.shape[1] != self.config.latent_channels:
            raise ValueError("hidden_states must be [B, latent_channels, H, W].")
        batch, _, height, width = hidden_states.shape
        if encoder_hidden_states.ndim != 3 or encoder_hidden_states.shape[0] != batch:
            raise ValueError("encoder_hidden_states must be [B, S, text_dim].")
        mask = encoder_attention_mask
        if mask is None:
            mask = torch.ones(
                encoder_hidden_states.shape[:2], dtype=torch.bool, device=hidden_states.device
            )
        times = torch.as_tensor(timestep, device=hidden_states.device, dtype=torch.float32).reshape(-1)
        times = times.expand(batch)
        half = self.config.freq_dim // 2
        freq = 10000 ** (-torch.arange(half, device=times.device, dtype=torch.float64) / half)
        phase = times.double()[:, None] * freq
        embedding = torch.cat((phase.cos(), phase.sin()), -1).float()
        time = self.time_embedding(embedding.to(COMPUTE_DTYPE)).float()
        mod = self.shared_block_modulation(time.to(COMPUTE_DTYPE)).view(batch, 6, -1)
        head = self.video_head_modulation(time.to(COMPUTE_DTYPE)).view(batch, 2, -1)
        outputs = []
        for index in range(batch):
            context = encoder_hidden_states[index : index + 1, mask[index].bool()]
            if context.shape[1] == 0:
                raise ValueError("Each sample needs at least one valid text token.")
            video = hidden_states[index : index + 1].flatten(2).transpose(1, 2)
            video = self.video_embedder(video.to(COMPUTE_DTYPE).contiguous())
            rope = self.rope(height, width, context.shape[1], hidden_states.device).to(video.dtype)
            text = self.text_embedder(context.to(COMPUTE_DTYPE), rope[:, height * width :])
            x = torch.cat((video, text), 1)
            for layer_index, layer in enumerate(self.layers):
                if layer_index not in skip_layers:
                    x = layer(x, rope, mod[index : index + 1])
            x = self.final_norm_video(x[:, : height * width].to(COMPUTE_DTYPE))
            x = x * (1 + head[index : index + 1, 1, None]) + head[index : index + 1, 0, None]
            x = self.final_linear_video(x.to(COMPUTE_DTYPE))
            outputs.append(x.transpose(1, 2).reshape(1, -1, height, width))
        sample = torch.cat(outputs)
        return Transformer2DModelOutput(sample=sample) if return_dict else (sample,)

    module._Block.forward = block_forward
    module._TextProjection.forward = text_projection_forward
    module.MOSSImageTransformer.forward = transformer_forward
