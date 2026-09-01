# FireRedTTS2 — judge report (wave 1)

Repo: `<GAUNTLET_CORPUS_ROOT>/FireRedTTS2` @ `404f3f6`.
Prod tree read in full (21 `.py` files, ~4.0 kLoC): `fireredtts2/` package,
`gradio_demo.py`, `setup.py`, `bin/finetune_example/`. `fireredtts2/codec/audio.py`
is vendored HuggingFace code (Apache header) and is judged lightly.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | fireredtts2/fireredtts2.py:383 | none | `FireRedTTS2_Stream` (lines 383-627, 245 lines = 39% of the file) has zero references anywhere in the repo — no import, no instantiation, not in `__init__.py`. A whole streaming subsystem carried as dead weight. | `class FireRedTTS2_Stream(FireRedTTS2):` |
| P1-2 | fireredtts2/fireredtts2.py:385 | #11 | `FireRedTTS2_Stream.generate` (385-465) is a copy of `FireRedTTS2.generate` (138-208): lines 394-423 are verbatim identical to 148-178; only the decode branch differs. | `max_generation_len = int(max_audio_length_ms / 80)` |
| P1-3 | fireredtts2/fireredtts2.py:467 | #11 | `FireRedTTS2_Stream.generate_single` (467-510) copies `FireRedTTS2.generate_single` (210-264); lines 470-487 verbatim match 213-231. | `self._model.reset_caches()` |
| P1-4 | fireredtts2/fireredtts2.py:513 | #11 | `FireRedTTS2_Stream.generate_dialogue` (513-565) copies 266-324, including the identical prompt-preparation block and the same two commented-out prints. | `text_list = process_text_list(text_list=text_list)` |
| P1-5 | fireredtts2/fireredtts2.py:190 | #11 | The 11-line autoregressive advance block (build `curr_tokens`, `curr_tokens_mask`, bump `curr_pos`) appears verbatim four times: 190-200, 245-255, 448-458, 500-510. | `curr_pos = curr_pos[:, -1:] + 1` |
| P1-6 | fireredtts2/fireredtts2.py:19 | none | `self.sample_rate = 16000` and `self.max_seq_len = 3100` are assigned in `__init__` and never read; line 72 hardcodes `16000` and lines 173 and 418 re-declare `max_seq_len = 3100` as locals. | `self.sample_rate = 16000` |
| P1-7 | fireredtts2/fireredtts2.py:17 | none | `self.use_bf16` is written at 17 and 50 and never read afterwards — the flag records a decision nothing consumes. | `self.use_bf16 = use_bf16` |
| P1-8 | fireredtts2/fireredtts2.py:22 | none | Nine bare `assert` statements (22, 23, 34-38, 280, 286) are the only validation of constructor preconditions; all vanish under `python -O`, turning a clear startup error into a downstream crash. | `assert os.path.exists(pretrained_dir)` |
| P1-9 | fireredtts2/fireredtts2.py:41 | none | `json.load(open(llm_config_path))` never closes the handle; every other JSON read in the repo (model.py:175, 212) uses a context manager. | `llm_config = json.load(open(llm_config_path))` |
| P1-10 | fireredtts2/fireredtts2.py:82 | #12 | `frame_tokens = []` / one `append` / `torch.cat(frame_tokens, dim=0)` — the list and the concat are no-ops on a single tensor. Same shape at 98-122. | `frame_tokens = []` |
| P1-11 | fireredtts2/fireredtts2.py:87 | none | Frame width `17` is a bare literal at 87, 88, 114, 115; the same quantity is `AUDIO_NUM_CODEBOOKS + 1` at posttrain_dataloader.py:169. One concept, two spellings, two homes. | `text_frame = torch.zeros(len(text_tokens), 17).long()` |
| P1-12 | fireredtts2/fireredtts2.py:106 | none | `batch_size=48` hardcoded at the only call site of `RedCodecInfer.encode`, whose own default is `96` (model.py:249) and which step2_extract_token.py:87 calls with `1`. Three unexplained values for one knob. | `batch_size=48,` |
| P1-13 | fireredtts2/fireredtts2.py:150 | none | `int(max_audio_length_ms / 80)` — the 80 ms frame duration is an unnamed literal here and at 396, and is the reciprocal of the `12.5` at step2_extract_token.py:90 and the `1280` at model.py:301. | `max_generation_len = int(max_audio_length_ms / 80)` |
| P1-14 | fireredtts2/fireredtts2.py:282 | #12 | `for i in range(len(prompt_wav_list))` indexes two parallel lists that `zip` would walk directly; repeated at 527. | `for i in range(len(prompt_wav_list)):` |
| P1-15 | fireredtts2/fireredtts2.py:280 | none | `assert len(prompt_wav_list) == len(prompt_text_list)` raises `TypeError: object of type 'NoneType' has no len()` when a caller passes wavs but leaves `prompt_text_list` at its `None` default — the signature permits exactly that. | `assert len(prompt_wav_list) == len(prompt_text_list)` |
| P1-16 | fireredtts2/fireredtts2.py:286 | #8 | The predicate "speaker tag is one of [S1]-[S4]" is re-validated at six sites: here, 298, 531, 543, spliter.py:284, and gradio_demo.py:144-147. It wants to be a validated `SpeakerTag` type. | `assert speaker in ["[S1]", "[S2]", "[S3]", "[S4]"]` |
| P1-17 | fireredtts2/fireredtts2.py:344 | none | `clean_text` runs three times on the same text: 339 on the whole string, then `split_text` calls it again (spliter.py:133), then 344 on each chunk. | `text = clean_text(text=text)` |
| P1-18 | fireredtts2/fireredtts2.py:352 | none | `while True:` retries generation until `gen_tokens.shape[2] > 18` with no attempt cap and no logging — a sampler that keeps producing short output hangs the process silently. Magic `18` unexplained. | `while True:` |
| P1-19 | fireredtts2/fireredtts2.py:358 | none | Commented-out debug branch in Chinese left in the retry loop; the same pattern at 296-297 and 541-542. | `# else:` |
| P1-20 | fireredtts2/fireredtts2.py:311 | none | Output sample rate `24000` is a bare literal here, at 561, and at gradio_demo.py:198, while the input rate lives in an unread `self.sample_rate`. Changing the vocoder rate requires finding three unrelated literals. | `audio_tensor.unsqueeze(0), 24000, 16000` |
| P1-21 | fireredtts2/fireredtts2.py:385 | none | `FireRedTTS2_Stream.generate` overrides a base method annotated `-> torch.Tensor` (138-147) with a generator and drops the annotation; `generate_single` does the same at 467. The subclass is not substitutable for its base. | `def generate(` |
| P1-22 | fireredtts2/fireredtts2.py:461 | none | If the first `generate_frame` returns all-zeros the loop breaks at 434 with `prev_sample` still `None`, and 461 dereferences it: `AttributeError: 'NoneType' object has no attribute 'unsqueeze'`. | `prev_sample.unsqueeze(-1),` |
| P1-23 | fireredtts2/fireredtts2.py:597 | none | `tokens` is declared once at 582 outside the `for text in text_list` loop, so the `len(tokens) > 2` warm-up guard only suppresses output for the first chunk; later chunks emit their first token immediately. | `if len(tokens) > 2:` |
| P1-24 | fireredtts2/fireredtts2.py:62 | #25 | A `RedCodecInfer` is bound to `torch_codec` then re-bound to `self._audio_tokenizer`; call sites read `self._audio_tokenizer.decode(...)`. The concept is renamed mid-delegation, so grepping "codec" misses every use. | `self._audio_tokenizer = torch_codec.to(device)` |
| P1-25 | fireredtts2/fireredtts2.py:1 | #29 | 627-line module, two public classes, the package's primary entry point — no module docstring and no cost note on `__init__`, which loads three checkpoints onto the GPU. | `import os` |
| P1-26 | fireredtts2/utils/spliter.py:38 | none | `"—"` is a duplicate dict key (already at line 26); the line-38 entry is silently dead. Confirmed by AST key comparison. | `"—": "",` |
| P1-27 | fireredtts2/utils/spliter.py:43 | #26 | `REPLACE_SYMBOL_REGEX` is a computed declaration: it joins `SYMBOLS_MAPPING.keys()` in insertion order, so the single em-dash at line 26 precedes `"————"` (36) and `"——"` (37) in the alternation and those two entries can never match. Reading the literal does not tell you what it does. | `"\|".join(re.escape(p) for p in SYMBOLS_MAPPING.keys())` |
| P1-28 | fireredtts2/utils/spliter.py:176 | #13 | `count_characters_chinese` is a pure rename of `len` — a hop that adds no meaning and no validation. | `def count_characters_chinese(text):` |
| P1-29 | fireredtts2/utils/spliter.py:181 | #11 | `split_by_punctuation_english` (181-195) and `split_by_punctuation_chinese` (198-212) are line-for-line identical except the regex character class. | `sentences = re.split(r"([.!?])", text)` |
| P1-30 | fireredtts2/utils/spliter.py:215 | #11 | `merge_sentences_english` (215-234) and `merge_sentences_chinese` (237-256) are identical except the join string and the counter function. | `test_chunk = current_chunk + " " + sentence` |
| P1-31 | fireredtts2/utils/spliter.py:80 | #11 | `break_text` (80-96) and `break_text_by_length` (98-113) share an identical generator skeleton and differ only in the yield condition. | `if utf_8_len(text) <= length:` |
| P1-32 | fireredtts2/utils/spliter.py:108 | #19 | `utf_8_len(curr)` re-encodes the whole accumulated string on every character of the loop, and `curr += char` reallocates it — quadratic in segment length on a hot path that runs per synthesis request. | `if utf_8_len(curr) >= length:` |
| P1-33 | fireredtts2/utils/spliter.py:116 | #30 | `add_cleaned` returns nothing and appends through an out-parameter; the caller's `segments` list is mutated invisibly from the signature. | `def add_cleaned(curr, segments):` |
| P1-34 | fireredtts2/utils/spliter.py:237 | none | `max_chars=100` is the declared default but the only caller passes `150` (line 269) — the default contradicts real usage and can never be exercised. `max_words=80` at 215 duplicates the `80` at 261. | `def merge_sentences_chinese(sentences, max_chars=100):` |
| P1-35 | fireredtts2/utils/spliter.py:263 | #8 | `^\[S\d+\]` accepts any speaker index, while line 284 (and every other layer) accepts only S1-S4, and the `text[:4]` slice at 282 mis-parses `[S10]` as `"[S10"`. Three disagreeing spellings of one invariant in one file. | `text = re.sub(r"^\[S\d+\]", "", text).strip()` |
| P1-36 | fireredtts2/utils/spliter.py:167 | none | Docstrings for the six functions at 166-256 are Chinese-only in an otherwise English codebase — an unnecessary comprehension cost for the majority of readers. | `"""检测文本是否包含中文字符"""` |
| P1-37 | fireredtts2/utils/spliter.py:1 | #29 | 289-line text-normalisation module with no module docstring; nothing states the pipeline order or that `split_text` re-cleans its input. | `import re` |
| P1-38 | fireredtts2/llm/utils.py:225 | #11 | `load_model` (225-258) and `load_llm_model` (261-294) are byte-identical except `configs["models"]` vs `configs["llm_models"]` — 34 lines duplicated to paper over a config-key rename between the training config and `config_llm.json`. | `backbone_flavor=configs["models"]["backbone_flavor"],` |
| P1-39 | fireredtts2/llm/utils.py:233 | #28 | The docstring documents `model_name_or_checkpoint_path` and `decoder_loss_weight`; neither is a parameter of `load_model(configs, checkpoint_path, device)`. Repeated verbatim at 269 and 271. | `model_name_or_checkpoint_path: Name or path of pretrained model` |
| P1-40 | fireredtts2/llm/utils.py:300 | #9 | Four mutable default arguments on one signature. Textbook shared-mutable-default hazard. | `scalars={},` |
| P1-41 | fireredtts2/llm/utils.py:226 | #1 | `configs` is unannotated (implicit `Any`) and then indexed two levels deep at 239-244; every caller must read the body to learn the required schema. | `configs,` |
| P1-42 | fireredtts2/llm/utils.py:227 | #1 | `checkpoint_path: Union[str, Path] = None` — the default is outside the declared type; `Optional` is meant. Same at 263. | `checkpoint_path: Union[str, Path] = None,` |
| P1-43 | fireredtts2/llm/utils.py:324 | none | Bare `except:` swallows every exception (including `KeyboardInterrupt`) and prints a name; line 327 then divides by `num`, which is `0` when no parameter has a gradient. | `except:` |
| P1-44 | fireredtts2/llm/utils.py:331 | #11 | `read_jsonl` is never called from anywhere, and is byte-identical to step2_extract_token.py:33-41 and step3_write_arrow.py:31-39. Three copies, one of them dead. | `def read_jsonl(path):` |
| P1-45 | fireredtts2/llm/utils.py:47 | none | `lr_lambda` is annotated `-> float` but the `"cosine"` branch returns a 0-dim `torch.Tensor` from `torch.cos`. | `return 0.5 * (` |
| P1-46 | fireredtts2/llm/utils.py:214 | #2 | `elif isinstance(m, nn.Parameter)` can never be true: `nn.Module.apply` only visits `nn.Module` instances, and `nn.Parameter` is a `Tensor` subclass, not a `Module`. The branch — and the comment at 204 promising it — is dead. | `elif isinstance(m, nn.Parameter):` |
| P1-47 | fireredtts2/llm/llm.py:52 | none | `sample_top_nsigma` (52-72) is never called, and carries an unedited IDE-generated docstring (`_summary_`, `_description_`, `_type_`) that documents nothing. | `"""_summary_` |
| P1-48 | fireredtts2/llm/llm.py:117 | #6 | `setup_caches` is annotated `-> torch.Tensor` and its docstring says "return a causal mask", but the body registers two buffers and returns `None`. Signature and doc both lie about the contract. | `def setup_caches(self, max_batch_size: int) -> torch.Tensor:` |
| P1-49 | fireredtts2/llm/llm.py:142 | #28 | The `forward` docstring instructs the reader that the method "will be added to the model with `model.forward = types.MethodType(forward, model)`" — `types` is never imported and no such assignment exists in the repo; `forward` is an ordinary method. | `This will be added to the model with model.forward = types.MethodType(forward, model)` |
| P1-50 | fireredtts2/llm/llm.py:262 | #28 | The `generate_frame` Args block documents a `mask:` parameter the signature does not have (249-256), and the line is left with an unclosed paren. | `mask: (batch_size, seq_len, max_seq_len` |
| P1-51 | fireredtts2/llm/llm.py:299 | none | `sample_topk(ci_logits, 10, 0.75)` silently discards the `topk` and `temperature` arguments the caller passed into `generate_frame`; they apply only to codebook 0. Every tuning knob exposed by `FireRedTTS2.generate` is inert for 15 of 16 codebooks. | `ci_sample = sample_topk(ci_logits, 10, 0.75)  # fix to 10 and 0.75` |
| P1-52 | fireredtts2/llm/llm.py:204 | none | The three shape comments at 204, 206, 207 say `audio_len//16` while line 203 slices `// 8`; line 202 (`# important change to 1/8`) records that the code changed and the comments did not. | `# [audio_len//16, n_codebooks-1, embed_dim]` |
| P1-53 | fireredtts2/llm/llm.py:233 | #11 | Both branches repeat the same four-line loss expression; only the `+ 0.01 * text_loss` term differs. | `2 * ((1 - self.decoder_loss_weight) * c0_loss` |
| P1-54 | fireredtts2/llm/llm.py:139 | #18 | `forward` is 108 lines narrated by eight labelled phases: `# embed tokens` (153), `# get targets and codebook embeddings` (156), `# get targets corresponding to text tokens` (162), `# retain just non-padding embeddings` (167), `# backbone forward pass` (171), `# predict text loss` (196), `# compute amortization` (201), `# concatenate backbone embeddings` (209). | `# embed tokens` |
| P1-55 | fireredtts2/llm/llm.py:9 | #6 | `_prepare_transformer` mutates its argument in place (replacing two submodules with `nn.Identity`) and also returns it, so callers cannot tell the input was destroyed. | `model.tok_embeddings = nn.Identity()` |
| P1-56 | fireredtts2/llm/llm.py:20 | #13 | `_index_causal_mask` binds one subscript to a temporary and returns it — a named hop over `mask[input_pos, :]` that adds only a docstring. | `r = mask[input_pos, :]` |
| P1-57 | fireredtts2/llm/modules.py:5 | #11 | Five functions (5-81) are the same 10-keyword call to `qwen2` with different numeric literals — 77 lines where one table keyed by flavour would do. | `def qwen2_200M() -> TransformerDecoder:` |
| P1-58 | fireredtts2/llm/modules.py:69 | #11 | `qwen2_7B` omits `tie_word_embeddings=True`, present in all four siblings. A divergence inside a clone family with nothing marking it as deliberate. | `rope_base=1000000.0,` |
| P1-59 | fireredtts2/llm/modules.py:13 | #14 | `max_seq_len=4096`, `attn_dropout=0.0`, `norm_eps=1e-6`, `rope_base=1000000.0` travel together, identical, across all five call sites — a shared-defaults group with no name. | `max_seq_len=4096,` |
| P1-60 | fireredtts2/codec/model.py:212 | none | The `with open(conf_path, "r") as f:` block never uses `f`; `RedCodec.from_config(conf_path)` opens the file itself at line 175. A file is opened, held, and discarded for nothing. | `with open(conf_path, "r") as f:` |
| P1-61 | fireredtts2/codec/model.py:152 | #14 | The same eight-submodule group (`ssl, ssl_adaptor, acoustic_encoder, downsample, rvq, upsample, semantic_decoder, acoustic_decoder`) is spelled out in three places: the `RedCodec.__init__` signature (152-162), the `from_config` return (185-194), and the `RedCodecInfer.__init__` super call (199-208). | `ssl: PretrainedWhisperEncoder,` |
| P1-62 | fireredtts2/codec/model.py:315 | #11 | Lines 315-321 in `decode` and 338-344 in `decode_one_token` are a verbatim seven-line token-to-latent prologue. | `tokens = tokens.permute(1, 0, 2)  # (B, nq, L) -> (nq, B, L)` |
| P1-63 | fireredtts2/codec/model.py:323 | none | `audio_length` is unpacked and never used; the caller loses the length the decoder computed. Same class of dead binding at 266 (`T`). | `audio, audio_length = self.acoustic_decoder(vq_out_feats, vq_out_length)` |
| P1-64 | fireredtts2/codec/model.py:27 | #1 | `ffn_dim: int = None` — default outside the declared type; same at 248 (`audio16k_length: torch.Tensor = None`) and 97. | `ffn_dim: int = None,` |
| P1-65 | fireredtts2/codec/model.py:301 | none | The `1280`-sample hop (and the `6 * 16000` chunk at 265) are unnamed literals that encode the same 12.5 Hz token rate spelled `12.5` at step2_extract_token.py:90 and `80` ms at fireredtts2.py:150. | `token_length = (audio16k_length / 1280).ceil().long()` |
| P1-66 | fireredtts2/codec/model.py:68 | #11 | `SslAdaptor._init_weights` (68-77) is byte-identical to `WhisperEncoder._init_weights` (whisper.py:263-272) — same 10 lines, same `std = 0.02`, in two modules. | `def _init_weights(self, module):` |
| P1-67 | fireredtts2/codec/model.py:177 | #6 | `PretrainedWhisperEncoder.from_pretrained()` is called with no argument, and its `pretrained_path` default is `None` (whisper.py:335), so this "pretrained" encoder is built with random weights. A factory named for loading that loads nothing. | `ssl = PretrainedWhisperEncoder.from_pretrained()` |
| P1-68 | fireredtts2/codec/model.py:1 | #29 | 376-line module defining five public classes including the codec entry point `RedCodecInfer` — no module docstring, and `from_pretrained` documents neither its checkpoint format nor its cost. | `import math` |
| P1-69 | fireredtts2/codec/whisper.py:49 | #11 | `WhisperSdpaAttention.forward` (49-79) and `forward_chunk` (81-118) share an identical five-line projection prologue (60-62 / 95-97) and an identical four-line output epilogue (75-78 / 114-117). | `query_states = self._shape(self.q_proj(hidden_states), tgt_len, bsz)` |
| P1-70 | fireredtts2/codec/whisper.py:142 | #11 | `WhisperEncoderLayer.forward` (142-162) and `forward_chunk` (164-192) are identical for 14 of 18 body lines; only the attention call differs. | `hidden_states = F.gelu(self.fc1(hidden_states))` |
| P1-71 | fireredtts2/codec/whisper.py:363 | #11 | `PretrainedWhisperEncoder.forward` (363-371) and `WhisperAcousticEncoder.forward` (412-420) are the same five statements; the only difference is the `torch.no_grad()` wrapper. | `mel, mel_length = self.feature_extractor(audio16k, audio16k_length)` |
| P1-72 | fireredtts2/codec/whisper.py:276 | #14 | The six-parameter mel group (`num_mels, sampling_rate, hop_length, n_fft, fmin, fmax`) is written out at five sites: 276-285, 294-302, 353-360, 378-383, 403-409. A `MelSpec` config object is missing. | `num_mels: int = 128,` |
| P1-73 | fireredtts2/codec/whisper.py:284 | none | `padding_value` is accepted, stored at 293, and never read anywhere in the repo. Dead parameter on a public constructor. | `padding_value=0.0,` |
| P1-74 | fireredtts2/codec/whisper.py:325 | none | `WhisperMelExtractor` is an `nn.Module` (276) but overrides `__call__` rather than `forward`, so PyTorch's hook and tracing machinery is bypassed for this module alone. | `def __call__(self, audio16k: torch.Tensor, audio16k_length: torch.Tensor):` |
| P1-75 | fireredtts2/codec/whisper.py:311 | none | `hann_window` (311) and `torch.from_numpy(self.mel_filters).to(device)` (316) are rebuilt and re-uploaded on every call; both are constants that belong in `register_buffer`. This runs once per audio chunk on the encode path. | `mel_filters = torch.from_numpy(self.mel_filters).type(torch.float32).to(device)` |
| P1-76 | fireredtts2/codec/whisper.py:126 | #1 | `ffn_dim: int = None` here, at 202, and at 388; `pretrained_path: str = None` at 335. Four public signatures whose defaults contradict their annotations. | `ffn_dim: int = None,` |
| P1-77 | fireredtts2/codec/whisper.py:6 | none | `Literal` is imported and never used. | `from typing import Optional, Literal` |
| P1-78 | fireredtts2/codec/decoder.py:8 | #11 | `ResnetBlock` (8-63) and `CausalResnetBlock` (105-148) duplicate the same constructor shape, the same `out_channels is None` idiom, the same `nin_shortcut` branch, and the same three-line `forward`. | `out_channels = in_channels if out_channels is None else out_channels` |
| P1-79 | fireredtts2/codec/decoder.py:175 | #11 | `VocosBackbone` (175-221) and `CausalVocosBackbone` (225-274) are the same class with `Causal*` substituted for three members and a different mask function; `forward` bodies match line for line. | `self.final_norm = nn.LayerNorm(embed_dim, eps=1e-6)` |
| P1-80 | fireredtts2/codec/decoder.py:350 | #11 | `ISTFT.forward` (350-405) and `ISTFT.forward_chunk` (407-468) duplicate the ifft, windowing, fold, envelope and normalise steps — the streaming variant is a fork, not a specialisation. | `ifft = torch.fft.irfft(spec, self.n_fft, dim=1, norm="backward")` |
| P1-81 | fireredtts2/codec/decoder.py:503 | #11 | `ISTFTHead.forward` lines 503-518 and `forward_chunk` lines 534-544 are verbatim identical, comments included. | `mag = torch.clip(mag, max=1e2)  # safeguard to prevent excessively large magnitudes` |
| P1-82 | fireredtts2/codec/decoder.py:595 | #25 | The attribute holding an `ISTFTHead` is spelled `isift` — a transposition of `istft` — and is used under that name at 621 and 692. Grepping the class name never reaches its call sites. | `self.isift = ISTFTHead(embed_dim, hop_length * 4, hop_length, padding="same")` |
| P1-83 | fireredtts2/codec/decoder.py:492 | none | `ISTFTHead.forward` is annotated `-> torch.Tensor` but returns the 2-tuple `(audio, audio_length)` at 521, and the docstring at 501 describes only the first element. | `def forward(self, x: torch.Tensor, x_len: torch.Tensor) -> torch.Tensor:` |
| P1-84 | fireredtts2/codec/decoder.py:599 | #11 | `AcousticDecoder._init_weights` is the third copy of the weight-init idiom (cf. model.py:68, whisper.py:263) but drops the `if module.bias is not None` guard its siblings have — `nn.init.constant_(None, 0)` on any bias-free `Conv1d`. | `nn.init.constant_(m.bias, 0)` |
| P1-85 | fireredtts2/codec/decoder.py:456 | none | The `window_envelope > 1e-11` assertion that guards the division at 403 is commented out on the streaming path, where the same division happens at 457 unguarded. | `# assert (window_envelope > 1e-11).all()` |
| P1-86 | fireredtts2/codec/decoder.py:510 | none | Seven lines of commented-out alternative implementation plus its rationale (510-517), duplicated again at 541 in `forward_chunk`. | `# phase = torch.atan2(y, x)` |
| P1-87 | fireredtts2/codec/decoder.py:12 | #1 | Twelve parameters across this file declare a non-Optional type with a `None` default: 12, 93, 109, 150, 279, 280, 281, 408, 524, 661, 663, 664. Every caller must read the body to learn `None` is legal. | `out_channels: int = None,` |
| P1-88 | fireredtts2/codec/decoder.py:611 | none | The upsample factor `2` is hardcoded at 611 and 617 and must stay in lockstep with `stride=2` at 574 — an invariant spread over three unlinked literals. | `target_length = x.shape[1] * 2` |
| P1-89 | fireredtts2/codec/decoder.py:1 | #29 | 700-line module — the largest in the repo — with eight public classes and no module docstring; nothing states the nonstreaming/streaming pairing that organises it. | `import torch` |
| P1-90 | fireredtts2/codec/rvq.py:8 | #13 | `WNConv1d` and `WNConvTranspose1d` are single forwarding calls that add nothing beyond the `weight_norm` wrap, and `WNConvTranspose1d` is never called anywhere in the repo. | `def WNConvTranspose1d(*args, **kwargs):` |
| P1-91 | fireredtts2/codec/rvq.py:8 | #1 | `*args, **kwargs` on a module-level public factory: the accepted arguments are undiscoverable without reading `nn.Conv1d`. | `def WNConv1d(*args, **kwargs):` |
| P1-92 | fireredtts2/codec/rvq.py:96 | #1 | `rvq_dim=None` is unannotated among annotated siblings, and `output_dim: int = None` declares a type its default violates — both are then used in `!=` comparisons that decide the module topology (110-119). | `rvq_dim=None,  # RVQ dimension.` |
| P1-93 | fireredtts2/codec/rvq.py:137 | #12 | `enumerate` is used for an index that is never read; `for quantizer in self.quantizers` is the whole loop. Same at 128 (`for i in range(num_quantizers)`). | `for i, quantizer in enumerate(self.quantizers):` |
| P1-94 | fireredtts2/codec/rvq.py:88 | #28 | The comment documents a four-value return (`z_q, commit_loss, indices, z`) while line 89 returns two; `commit_loss` does not exist in this function. | `# z_q: (B, D, T), commit_loss: (B), indices: (B, T), z: (B, D', T)` |
| P1-95 | fireredtts2/codec/audio.py:134 | #2 | `norm is not None` is implied by `norm == "slaney"` — the first conjunct is provably redundant. (Vendored HF code, but the redundancy is real.) | `if norm is not None and norm == "slaney":` |
| P1-96 | fireredtts2/codec/utils.py:15 | #13 | `make_nonpad_mask` is a single forwarding call with a `~`; it prices a hop and an import in six call sites to save one character. | `return ~make_pad_mask(lengths, max_len)` |
| P1-97 | gradio_demo.py:13 | #9 | `model` is a module-level mutable written by `initiate_model` (17-23) and read by `dialogue_synthesis_function` (191); nothing on the read path checks it was initialised, and the annotation `FireRedTTS2` excludes its actual initial value `None`. | `model: FireRedTTS2 = None` |
| P1-98 | gradio_demo.py:116 | #9 | `global_lang` is a module-level mutable mutated by a nested UI callback (272) and read by `i18n` (121) — the entire i18n layer communicates through a global. | `global_lang: Literal["zh", "en"] = "zh"` |
| P1-99 | gradio_demo.py:120 | none | `global global_lang` in a function that only reads the name — the declaration has no effect and misleads the reader into expecting a write. | `global global_lang` |
| P1-100 | gradio_demo.py:164 | none | `spk1_prompt_text != ""` is `True` for `None`, which the signature explicitly allows (156). A `None` prompt text passes the completeness gate at 169 and reaches `text.strip()` in `check_monologue_text` (125) as an `AttributeError`. | `spk1_prompt_text != "",` |
| P1-101 | gradio_demo.py:179 | #8 | The extraction regex admits `[S0]` and `[S5]`-`[S9]`, which `check_dialogue_text` (139-150) then rejects — but `process_text_list` (spliter.py:284) would have crashed on a bare assert. Three layers, three different spellings of the speaker-tag rule. | `re.findall(r"(\[S[0-9]\][^\[\]]*)", target_text)` |
| P1-102 | gradio_demo.py:143 | #8 | The four speaker tags are hand-unrolled into four calls; adding `[S5]` means editing this disjunction plus five other sites. | `check_monologue_text(text, "[S1]")` |
| P1-103 | gradio_demo.py:186 | none | `progress_bar` is assigned and never used — `gr.Progress(track_tqdm=True)` is constructed for its side effect, which is not what the binding says. | `progress_bar = gr.Progress(track_tqdm=True)` |
| P1-104 | gradio_demo.py:3 | none | `tqdm` (3) and `Tuple` (5) are imported and never used. | `from tqdm import tqdm` |
| P1-105 | gradio_demo.py:202 | #18 | `render_interface` is 142 lines narrated by labelled phases: `# ======= UI =======` (204), `# ==== Speaker1 Prompt ====` (223), `# ==== Speaker2 Prompt ====` (237), `# ==== Text input ====` (251), `# ======= Action =======` (268). Each banner marks a function that was never extracted. | `# ======================== UI ========================` |
| P1-106 | gradio_demo.py:124 | #1 | `prefix: str = None` — default outside the declared type on a function whose whole job is prefix validation. | `def check_monologue_text(text: str, prefix: str = None) -> bool:` |
| P1-107 | setup.py:3 | none | `setup()` declares no `install_requires`, and `requirements.txt` omits `torch`, `torchaudio`, `numpy`, `tqdm`, `datasets`, `huggingface_hub`, `pyyaml` and `tokenizers` — all imported by the package. `pip install .` produces a package that cannot import. | `setup(name="fireredtts2", version="0.1", packages=find_packages())` |
| P1-108 | bin/finetune_example/posttrain.py:22 | #1 | `config: dict` on the module's only entry point, then indexed as `config["train"]["..."]` at thirteen distinct keys. The contract is discoverable only by reading 200 lines. | `def train(args: argparse.Namespace, config: dict, trial: optuna.Trial = None):` |
| P1-109 | bin/finetune_example/posttrain.py:41 | #30 | `config["train"][...]` is reached two levels deep at 15 call sites (41, 49-51, 57, 59, 66-67, 71-73, 109, 112-114, 125, 128, 170, 191) — the training loop is coupled to the JSON layout rather than to a settings object. | `logs_folder = config["train"]["logs_folder"]` |
| P1-110 | bin/finetune_example/posttrain.py:22 | none | `trial` is never referenced in the body; `optuna` (line 8) is imported solely to annotate a dead parameter, and the docstring promises a sweep that does not exist. | `trial is only used when we are sweeping hyperparameters.` |
| P1-111 | bin/finetune_example/posttrain.py:3 | none | `pickle` (3), `yaml` (4), `Path` (5), `tqdm` (6) and `GradScaler` (11) are imported and never used. | `import pickle` |
| P1-112 | bin/finetune_example/posttrain.py:42 | none | `writer` is bound only inside `if accelerator.is_main_process:` and used at 180 under a second, separate guard — a conditionally-bound name whose safety depends on two conditions staying in sync. | `if accelerator.is_main_process:` |
| P1-113 | bin/finetune_example/posttrain.py:48 | none | `valloader` is unpacked and never used; `create_dataloaders` eagerly builds and loads the entire validation dataset (posttrain_dataloader.py:298) to produce it. | `trainloader, valloader = create_dataloaders(` |
| P1-114 | bin/finetune_example/posttrain.py:54 | none | Three disagreeing values for one knob: `config_finetune_1.5b_0.2b.json` declares `num_workers: 4`, this call passes `8`, and the train loader hardcodes `12` (posttrain_dataloader.py:304), discarding both. | `num_workers=8,` |
| P1-115 | bin/finetune_example/posttrain.py:22 | #28 | `keep_ckpts`, `val_every`, `gen_every` and `num_workers` are documented as tunable knobs in `bin/finetune_example/tutorial.md` (Part 2 Step 1 config block) and present in the shipped config, but no code reads them. `models.sample_rate` is likewise dropped by `load_model`, which builds `ModelArgs` without it (llm/utils.py:238-246). | `"keep_ckpts": 10,` |
| P1-116 | bin/finetune_example/posttrain.py:221 | #6 | `train` returns `None`; the caller binds it to `final_val_loss`. The name asserts a validation loss that the function never computes — validation is absent from the loop entirely. | `final_val_loss = train(args, config)` |
| P1-117 | bin/finetune_example/posttrain.py:97 | #11 | The four-accumulator block (`total_loss`, `total_text_loss`, `total_c0_loss`, `total_c_loss`) is written out four times: initialised at 97-100, appended at 137-140, reset at 185-188 and again at 198-201. Four variables that always move together. | `total_loss = 0.0` |
| P1-118 | bin/finetune_example/posttrain_dataloader.py:231 | none | `BucketSampler` (231-285, 55 lines) is never instantiated — both `batch_sampler=` lines are commented out (301, 311) — and `TokenizedDataset.get_seq_len` (43-45), its only data source, is dead with it. | `class BucketSampler(Sampler):` |
| P1-119 | bin/finetune_example/posttrain_dataloader.py:27 | none | Unedited IDE-stub docstrings (`_summary_`, `_description_`, `_type_`) at 27-32, 68-73 and 95-102; the third documents four parameters of the file's most intricate function with nothing at all. | `"""_summary_` |
| P1-120 | bin/finetune_example/posttrain_dataloader.py:106 | #18 | `interleave` is narrated by literal step comments `# step1.` (106), `# step2.` (111), `# step3.` (123), `# step4.` (147) — four numbered phases inside one 104-line function. | `# step1.` |
| P1-121 | bin/finetune_example/posttrain_dataloader.py:127 | none | When the `while True` search reaches `start_index == end_index` it breaks at 140 **without** applying the slice, so `audio_segment_index` keeps its original over-length contents and the `AUDIO_MAX_LEN` bound the loop exists to enforce silently does not hold. | `print("start_index == end_index", start_index, "==", end_index)` |
| P1-122 | bin/finetune_example/posttrain_dataloader.py:134 | none | `new_total_audio_len` is measured over `audio_segment_index[start_index]` through `[end_index]` inclusive, but the accepted slice at 143 is `[start_index:end_index]`, which drops the last segment that was measured. Measurement and action disagree by one element. | `audio_segment_index[end_index][-1] - audio_segment_index[start_index][0]` |
| P1-123 | bin/finetune_example/posttrain_dataloader.py:292 | #15 | `device` is threaded through `create_dataloaders` into `TokenizedDataset.__init__` (26), stored at 38, and never read — the comment `# for debug` documents that it is dead rather than removing it. | `device,  # for debug` |
| P1-124 | bin/finetune_example/posttrain_dataloader.py:304 | none | The `num_workers` parameter (294) is ignored for the train loader — the one that matters — while the unused val loader honours it (314). A parameter accepted and silently dropped. | `num_workers=12,` |
| P1-125 | bin/finetune_example/posttrain_dataloader.py:67 | #11 | `get_index` (67-91) is duplicated as the first half of `recovery_debug` (step3_write_arrow.py:168-181) — same variable names, same loop, same accumulator pattern. | `end_audio_segment_index = start_audio_segment_index + audio_segment_len[i]` |
| P1-126 | bin/finetune_example/posttrain_dataloader.py:80 | #12 | `for i in range(len(...))` walking two parallel lists by index, three times in this file (80, 151, 174) — `zip` expresses it directly. | `for i in range(len(audio_segment_len)):` |
| P1-127 | bin/finetune_example/posttrain_dataloader.py:104 | none | `TOTAL_MAX_LEN = 3100` is a function-local constant restating the sequence budget hardcoded at fireredtts2.py:20, 173 and 418. The training and inference limits can drift apart with no test catching it. | `TOTAL_MAX_LEN = 3100` |
| P1-128 | bin/finetune_example/posttrain_dataloader.py:1 | none | `os` (1), `Path` (2), `Array2D`/`Sequence`/`Features`/`Value` (14) and `ArrowWriter` (15) are imported and never used. | `import os` |
| P1-129 | bin/finetune_example/data_preparation/step1_create_meta.py:2 | none | `sys` (2), `tarfile` (3), `glob` (5) and `re` (6) are imported and never used. | `import sys` |
| P1-130 | bin/finetune_example/data_preparation/step1_create_meta.py:45 | none | Three `open` calls with no context manager (45, 49) and manual `close` (47, 78); an exception inside the 51-76 loop leaks the output handle and leaves a truncated JSONL. | `f_in = open(meta_path)` |
| P1-131 | bin/finetune_example/data_preparation/step1_create_meta.py:69 | none | The speaker tag `"[S_DIALOG_1]"` is hardcoded here, must match one of the 126 literals in llm/utils.py:115-124, and is not validated anywhere before the tokenizer silently splits an unknown tag into subwords. | `segment_dict["speaker"] = "[S_DIALOG_1]"` |
| P1-132 | bin/finetune_example/data_preparation/step2_extract_token.py:2 | none | Fifteen unused imports: `sys`, `tarfile`, `glob`, `re`, `ProcessPoolExecutor`, `librosa`, `SDPBackend`, `sdpa_kernel`, `AutoModelForSpeechSeq2Seq`, `AutoProcessor`, `pipeline`, `load_dataset`, `Path`, `pad_sequence`, `BytesIO` — the header advertises a Whisper ASR pipeline and multiprocessing the file does not contain. | `from transformers import AutoModelForSpeechSeq2Seq, AutoProcessor, pipeline` |
| P1-133 | bin/finetune_example/data_preparation/step2_extract_token.py:27 | #9 | `warnings.filterwarnings("ignore")` and `torch.set_num_threads(2)` run at import time, so merely importing this module reconfigures process-global state for everything else in the interpreter. | `warnings.filterwarnings("ignore")` |
| P1-134 | bin/finetune_example/data_preparation/step2_extract_token.py:44 | none | `split_list_into_chunks` is never called. | `def split_list_into_chunks(lst, chunk_size):` |
| P1-135 | bin/finetune_example/data_preparation/step2_extract_token.py:74 | #12 | `jsonl[:-6] + "_token" + ".jsonl"` reimplements `Path.with_stem`/`os.path.splitext` and silently mangles any input path not ending in exactly `.jsonl`. | `output_jsonl_file = jsonl[:-6] + "_token" + ".jsonl"` |
| P1-136 | bin/finetune_example/data_preparation/step2_extract_token.py:67 | none | `torch.device("cuda")` is hardcoded with no CPU fallback and no flag, while the sibling scripts take everything else from argparse. | `device = torch.device("cuda")` |
| P1-137 | bin/finetune_example/data_preparation/step2_extract_token.py:95 | none | `token.squeeze()` removes every size-1 dimension, so a clip that quantises to a single frame collapses from `(1, nq, 1)` to `(nq,)` and is written with the wrong rank. `squeeze(0)` is meant. | `one_token = token.squeeze().cpu().numpy().tolist()` |
| P1-138 | bin/finetune_example/data_preparation/step3_write_arrow.py:87 | none | `is_useful` is set to `False` at 94 for the "too many speakers" case and never read again — the row is written to the arrow file anyway at 144. The filter this flag exists to implement does not exist. | `is_useful = False` |
| P1-139 | bin/finetune_example/data_preparation/step3_write_arrow.py:42 | #15 | `get_speaker_dict` builds a full speaker-to-tag mapping (42-55) of which the caller uses only `len()` (92); the mapping is discarded and line 113 formats the prompt with the raw `seg["speaker"]` instead. A rich value demanded for one scalar. | `if len(speaker_dict) > 5:` |
| P1-140 | bin/finetune_example/data_preparation/step3_write_arrow.py:157 | none | `recovery_debug` (157-208) and `read_test` (211-241) — 85 lines — are reachable only from the commented-out call at 272. | `# read_test(dataset_dir=args.dataset_dir)` |
| P1-141 | bin/finetune_example/data_preparation/step3_write_arrow.py:226 | none | `idx = 1` is a dead store, immediately overwritten by the loop variable on the next statement. | `idx = 1` |
| P1-142 | bin/finetune_example/data_preparation/step3_write_arrow.py:190 | none | `.reshape([16, -1])` hardcodes the codebook count that posttrain_dataloader.py:17 names `AUDIO_NUM_CODEBOOKS`, and that the shipped config sets as `audio_num_codebooks: 16`. Three homes for one number. | `].reshape([16, -1])` |
| P1-143 | bin/finetune_example/data_preparation/step3_write_arrow.py:113 | #11 | The prompt template `speaker + "<\|text_start\|>" + text + "<\|text_end\|>"` is duplicated verbatim at fireredtts2.py:85. Training-time and inference-time formatting can drift silently, which is the one bug class a TTS repo cannot detect from its own outputs. | `text = speaker + "<\|text_start\|>" + text + "<\|text_end\|>"` |
| P1-144 | bin/finetune_example/data_preparation/step3_write_arrow.py:92 | #8 | The speaker cap is `5` here while every other layer caps at four tags `[S1]`-`[S4]` (fireredtts2.py:286, spliter.py:284, gradio_demo.py:143). | `if len(speaker_dict) > 5:` |
| P1-145 | bin/finetune_example/data_preparation/step3_write_arrow.py:126 | none | Filter thresholds `1.5`, `30` and `7.5` are inline literals immediately below two named constants (`MAX_AUDIO_DURATION`, `MAX_TEXT_TOKEN_LEN`) that do the same job — the file disagrees with itself about whether tuning knobs get names. | `if audio_text_ratio < 1.5 and len(text_tokens) > 30:` |
| P1-146 | bin/finetune_example/data_preparation/step3_write_arrow.py:139 | none | `total_len` counts audio *frames* (`audio_tokens.shape[-1]`) while `audio_segment_len` at 135 stores the *flattened* length (16x frames). Two length columns of the same row are written in different units, and `interleave` divides one by `AUDIO_NUM_CODEBOOKS` (posttrain_dataloader.py:121) while `BucketSampler` would have sorted on the other. | `total_len += audio_tokens.shape[-1] + len(text_tokens) + 1` |
| P1-147 | bin/finetune_example/data_preparation/step3_write_arrow.py:4 | none | Nine unused imports: `ProcessPoolExecutor` (4), `random` (6), `librosa` (7), `torchaudio` (9), `pickle` (10), `Array2D` (15), `Audio` (17), `AutoTokenizer` (20), `TemplateProcessing` (21). | `from tokenizers.processors import TemplateProcessing` |
| P1-148 | bin/finetune_example/posttrain.py:121 | none | Chinese-only comments (121, 124, 184) in an otherwise English file, mirroring fireredtts2.py:309/559 and step3_write_arrow.py:186. | `# 梯度传导` |

## Phase 2 — audit finding verdicts

135 findings. High-count rules grouped (representative sites + count, one
verdict); every exception split to its own row so all 135 are accounted for.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| 32x free-function/velcro: decoder.py:49, model.py:53, rvq.py:132, whisper.py:412, llm/llm.py:311, +27 | #22 | heuristic | fp | Every flagged method is an `nn.Module.forward`/compute method or its private helper; the "public interface" it touches is the module's own registered submodules/`self.config`, its de-facto private composition. `forward` is a framework-mandated call operator, not relocatable to a free function, so the ideal is not violated. `generate_dialogue` is the only arguable real; still fp. |
| 16x missing module docstring: fireredtts2.py:1, decoder.py:1, whisper.py:1, spliter.py:1, gradio_demo.py:1, +11 | #29 | heuristic | real | Every listed module opens on `import ...` with no top-loading docstring; verified by reading first lines. |
| 12x heavy entry no cost doc: fireredtts2.py:139/210/267/327/385/467/513/568, dataloader.py:288, decoder.py:276, gradio_demo.py:153/202 | #29 | heuristic | real | Each runs the model / loads full datasets / streams / builds the whole UI and documents no cost. |
| fireredtts2/utils/spliter.py:132 | #29 | heuristic | fp | `split_text` is a 31-line pure text-splitting function with no I/O or model cost; "heavy entry point" is a size-proxy misfire. |
| fireredtts2/codec/audio.py:102 | #29 | heuristic | fp | `mel_filter_bank` is a pure-numpy filterbank build (vendored HF), cheap and one-shot; not a costly entry point. |
| 17x always-True/False comparison: decoder.py:18/94/114/158/287/303/430/460/633, model.py:259, whisper.py:100/136/345, llm/utils.py:210, gradio_demo.py:18/127/130 | #2 | proved (15) / heuristic (2) | fp | Every site is a `param: T = None` default (or a None-initialised global / bias-free `.bias`) carrying a non-Optional annotation. The oracle trusts the annotation and calls the `is None`/`is not None` check redundant, but the value is None at runtime and the branch is live; removing it breaks the default. Unsound proved tier: the annotation lies, the check is necessary. Systematic FP class on `= None` defaults. |
| 13x structural clone: llm/utils.py:225/261/331, modules.py:5/21/37/53, model.py:68, whisper.py:263, spliter.py:181/198, step2:33, step3:31 | #11 | indexed | real | Genuine near-identical copies: `load_model`/`load_llm_model`, four `qwen2_*` bodies, `_init_weights` across two modules, `read_jsonl` x3, `split_by_punctuation_*` pair. |
| 6x data clump: decoder.py:176 (x2), model.py:20 (x2), fireredtts2.py:139 (x2) | #14 | indexed | real | The flagged groups (`embed_dim,num_heads,dropout[,num_layers]`; `attn_dropout,dropout,embed_dim,ffn_dim,num_heads[,num_layers]`; `temperature,topk[,text/context]`) genuinely recur across the stated signatures. |
| fireredtts2/llm/utils.py:316 | #6 | indexed | real | `get_grad_norm` is accessor-named but its `except:` prints to stdout, a write effect the name hides. |
| bin/.../step1_create_meta.py:13 | #6 | indexed | real | `get_uttrid2path` is a get_ accessor performing filesystem I/O (`os.listdir`), effect outside the accessor contract. |
| bin/.../step2_extract_token.py:33 | #6 | indexed | fp | `read_jsonl` honestly announces file reading in its name; reading a jsonl is the contract, not a hidden effect. |
| bin/.../step3_write_arrow.py:31 | #6 | indexed | fp | Same `read_jsonl`; the read_ name is honest about the I/O. |
| fireredtts2/llm/utils.py:331 | #6 | indexed | fp | Same `read_jsonl`; honest name. |
| bin/.../step3_write_arrow.py:211 | #6 | indexed | fp | `read_test` is debug code whose read_ name already implies I/O; the print/load effects match the name. |
| 4x proof-lift: step1_create_meta.py:13, llm/utils.py:297 (x2), gradio_demo.py:16 | #5 | indexed | real | Each proposes a correct type (`wav_dir: str`, `global_step: int`, `audio_sampling_rate: int`, `device: str`) for a currently-unannotated parameter whose single call site establishes it; counterfactual application verified clean. |
| 4x section comments: dataloader.py:106, fireredtts2.py:40, spliter.py:136, gradio_demo.py:204 | #18 | heuristic | real | Each narrates numbered/banner phases (`step1..4`, `==== ... ====`, split-rule list, UI/Action banners) marking uncut function boundaries. |
| 4x mutable default: llm/utils.py:300/301/302/303 | #9 | heuristic | real | `summarize` declares `scalars/histograms/images/audios = {}`, four shared mutable defaults. |
| 3x Demeter reach: model.py:71, whisper.py:232/266 | #30 | heuristic | real | Each is a genuine 3-hop reach mutating through a parameter/submodule (`module.weight.data.normal_`, `self.embed_positions.weight.copy_`); idiomatic torch init, but the deep reach is real. |
| bin/.../posttrain.py:22 | #1 | heuristic | real | `train(config: dict)`, bare dict on the module's only entry point, indexed at 13 keys; caller must read the body for the schema. |
| fireredtts2/codec/rvq.py:8 | #1 | heuristic | real | `WNConv1d(*args, **kwargs)`, opaque kwargs on a public factory; accepted args undiscoverable. |
| fireredtts2/codec/rvq.py:12 | #1 | heuristic | real | Same opaque `**kwargs` on `WNConvTranspose1d`. |
| 2x symbol price: whisper.py:121, fireredtts2.py:15 | #27 | indexed | real | `WhisperEncoderLayer` (fan-in 5) and `FireRedTTS2` (fan-in 3) each live in a 420/628-line module; readers pay the whole file per symbol. |
| 2x over-constrained param: dataloader.py:200, spliter.py:80 | #10 | indexed | real | `collate_fn` needs only `Iterable[dict]` (one pass) and `break_text` only `Collection` (membership); widening verified clean; concrete `List[dict]`/`set` over-demands. |
| 2x doc path: tutorial.md:50 (dataset_info.json, state.json) | #28 | indexed | fp | Both are prospective output files the tutorial instructs the reader to create (their JSON content is shown inline), not references to existing repo contents; non-resolution is by design. |
| fireredtts2/utils/spliter.py:176 | #13 | indexed | real | `count_characters_chinese` is a pure rename of `len`, a forwarding hop adding nothing. |
| fireredtts2/codec/whisper.py:42 | #13 | indexed | fp | `_shape` performs a non-trivial head-splitting `view().transpose().contiguous()` reshape reused 3x in the class; meaningful work, not a pointless forward. |
| 2x distributed invariant: decoder.py:105, llm/llm.py:86 | #21 | heuristic | real | `self.in_channels != self.out_channels` recurs in 3 methods of `CausalResnetBlock`; `next(self.parameters())` recurs in 3 methods of `Model`; both want encapsulating. |
| fireredtts2/utils/spliter.py:43 | #26 | heuristic | real | `REPLACE_SYMBOL_REGEX` is assembled by joining `SYMBOLS_MAPPING.keys()` into an alternation; a reader must execute the join (and know dict order) to know the members. |
| fireredtts2/utils/spliter.py:48 | #26 | heuristic | fp | `EMOJI_REGEX` is adjacent string-literal concatenation compiled once; statically readable, not code-assembled. |
| fireredtts2/fireredtts2.py:97 | #15 | heuristic | fp | `_tokenize_audio(audio)` uses `.shape`/`.to`, but `.to(device)` forwards the whole tensor to the encoder; `audio` is the payload, not a wallet whose few attributes are extracted. |

## Phase 3 — reconciliation

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | none | inventory-gap | No dead-code rule; #22/#29 fired on the class's methods, not its deadness. |
| P1-2 | #11 | threshold-miss | Clone detector shipped repo-wide; `Stream.generate` vs `generate` (80-line near-clone) under its similarity cutoff. |
| P1-3 | #11 | threshold-miss | `Stream.generate_single` near-clone under cutoff. |
| P1-4 | #11 | threshold-miss | `Stream.generate_dialogue` near-clone under cutoff. |
| P1-5 | #11 | threshold-miss | 11-line AR-advance block repeated 4x, below function-clone granularity. |
| P1-6 | none | inventory-gap | Unread instance attribute (dead store); no rule. |
| P1-7 | none | inventory-gap | Unread `use_bf16`; no rule. |
| P1-8 | none | inventory-gap | assert-as-validation; no rule. |
| P1-9 | none | inventory-gap | `open()` without context manager; no rule. |
| P1-10 | #12 | detector-miss | #12 (idiom-catalog) fired nowhere repo-wide; single-element list+cat no-op uncaught. |
| P1-11 | none | inventory-gap | Magic literal `17` duplicated; no rule. |
| P1-12 | none | inventory-gap | Magic `batch_size=48`; no rule. |
| P1-13 | none | inventory-gap | Frame-duration literal `80`; no rule. |
| P1-14 | #12 | detector-miss | `range(len(...))` over parallel lists; #12 silent repo-wide. |
| P1-15 | none | inventory-gap | `len(None)` latent crash; no rule. |
| P1-16 | #8 | detector-miss | #8 (primitive-obsession) fired nowhere; the 6-site speaker-tag predicate uncaught. |
| P1-17 | none | inventory-gap | Redundant triple `clean_text`; no rule. |
| P1-18 | none | inventory-gap | Uncapped `while True` retry; no rule. |
| P1-19 | none | inventory-gap | Commented-out debug; no rule. |
| P1-20 | none | inventory-gap | Magic `24000`; no rule. |
| P1-21 | none | inventory-gap | LSP substitutability break; no rule. |
| P1-22 | none | inventory-gap | `None.unsqueeze` latent crash; no rule. |
| P1-23 | none | inventory-gap | Loop-scope warm-up bug; no rule. |
| P1-24 | #25 | detector-miss | #25 (rename-delegation) fired nowhere; `torch_codec`->`_audio_tokenizer` rename uncaught. |
| P1-25 | #29 | covered | Module-docstring finding at fireredtts2.py:1. |
| P1-26 | none | inventory-gap | Duplicate dict key (dead entry); no rule. |
| P1-27 | #26 | covered | REPLACE_SYMBOL_REGEX finding at spliter.py:43. |
| P1-28 | #13 | covered | count_characters_chinese finding at spliter.py:176. |
| P1-29 | #11 | covered | split_by_punctuation clone finding at spliter.py:181. |
| P1-30 | #11 | threshold-miss | `merge_sentences_english`/`_chinese` is a distinct clone pair not grouped (sibling caught 17 lines away). |
| P1-31 | #11 | threshold-miss | `break_text`/`break_text_by_length` pair under clone cutoff. |
| P1-32 | #19 | detector-miss | #19 (linear-op-in-loop) fired nowhere; quadratic `utf_8_len(curr)` per-char uncaught. |
| P1-33 | #30 | detector-miss | Void-mutator half of #30 was cut; out-param append not flagged. |
| P1-34 | none | inventory-gap | Default contradicts sole caller; no rule. |
| P1-35 | #8 | detector-miss | #8 silent; `[S\d+]` vs S1-4 disagreement uncaught. |
| P1-36 | none | inventory-gap | Chinese-only docstrings; no rule. |
| P1-37 | #29 | covered | Module-docstring finding at spliter.py:1. |
| P1-38 | #11 | covered | load_model/load_llm_model clone at utils.py:225. |
| P1-39 | #28 | detector-miss | #28 fired only on file paths, not docstring param names; `model_name_or_checkpoint_path` mismatch uncaught. |
| P1-40 | #9 | covered | mutable-default finding at utils.py:300. |
| P1-41 | #1 | detector-miss | Unannotated `configs` (implicit Any) not flagged though #1 active. |
| P1-42 | #1 | detector-miss | `Union[str,Path]=None` (should be Optional); #1 targets Any/dict/kwargs, not None-defaults. |
| P1-43 | none | inventory-gap | Bare `except:` + div-by-zero; no rule. |
| P1-44 | #11 | covered | read_jsonl x3 clone includes utils.py:331. |
| P1-45 | none | inventory-gap | Return-type/Tensor mismatch; no rule. |
| P1-46 | #2 | covered | #2 fired at utils.py:210 in this function (that finding is itself an FP; my dead `nn.Parameter` branch at 214 is the real issue). |
| P1-47 | none | inventory-gap | Dead function + stub docstring; no rule. |
| P1-48 | #6 | detector-miss | Return-annotation lie (`-> Tensor` returns None); #6 keys on accessor names, missed. |
| P1-49 | #28 | detector-miss | Docstring names a non-existent `types.MethodType` mechanism; #28 doesn't check docstring prose. |
| P1-50 | #28 | detector-miss | Docstring documents a `mask:` param the signature lacks; uncaught. |
| P1-51 | none | inventory-gap | `topk`/`temperature` silently ignored; no rule. |
| P1-52 | none | inventory-gap | Stale `//16` shape comments; no rule. |
| P1-53 | #11 | threshold-miss | Two-branch loss near-dup, below function-clone granularity. |
| P1-54 | #18 | detector-miss | `forward` narrates 8 phases; #18 active elsewhere but missed this genuine instance. |
| P1-55 | #6 | detector-miss | Mutate-and-return hidden effect; #6 keys on accessor names, missed. |
| P1-56 | #13 | detector-miss | `_index_causal_mask` single-subscript forward; #13 active, missed. |
| P1-57 | #11 | covered | qwen2_* clone group at modules.py:5. |
| P1-58 | #11 | threshold-miss | qwen2_7B is the excluded near-clone (missing `tie_word_embeddings`), under the group's cutoff. |
| P1-59 | #14 | detector-miss | Shared default values across param-less functions; #14 needs param groups, out of reach. |
| P1-60 | none | inventory-gap | Opened-and-unused file handle; no rule. |
| P1-61 | #14 | threshold-miss | 8-submodule group across 3 sites; #14 fired on other clumps, this under its recurrence cutoff. |
| P1-62 | #11 | threshold-miss | decode/decode_one_token 7-line prologue under clone cutoff. |
| P1-63 | none | inventory-gap | Unused `audio_length` unpack; no rule. |
| P1-64 | #1 | detector-miss | `ffn_dim: int = None`, non-Optional-default; no exact rule (#1 is Any/dict/kwargs). |
| P1-65 | none | inventory-gap | Magic `1280`/`6*16000`; no rule. |
| P1-66 | #11 | covered | _init_weights clone at model.py:68. |
| P1-67 | #6 | detector-miss | `from_pretrained` loads nothing (misleading factory); #6 keys on accessor names. |
| P1-68 | #29 | covered | Module-docstring finding at model.py:1. |
| P1-69 | #11 | threshold-miss | WhisperSdpaAttention forward/forward_chunk prologue+epilogue clone under cutoff. |
| P1-70 | #11 | threshold-miss | WhisperEncoderLayer forward/forward_chunk (14/18 lines shared) under cutoff. |
| P1-71 | #11 | threshold-miss | Pretrained/Acoustic encoder forward pair under cutoff. |
| P1-72 | #14 | detector-miss | 6-param mel group across 5 sigs; #14 active, missed this clump. |
| P1-73 | none | inventory-gap | Dead `padding_value` param; no rule. |
| P1-74 | none | inventory-gap | `__call__` override bypassing `forward`; no rule. |
| P1-75 | none | inventory-gap | Per-call constant rebuild (perf); no rule. |
| P1-76 | #1 | detector-miss | Four non-Optional `=None` defaults; no exact rule. |
| P1-77 | none | inventory-gap | Unused import; no rule. |
| P1-78 | #11 | threshold-miss | ResnetBlock/CausalResnetBlock structural clone under cutoff. |
| P1-79 | #11 | threshold-miss | VocosBackbone/CausalVocosBackbone clone under cutoff. |
| P1-80 | #11 | threshold-miss | ISTFT forward/forward_chunk fork under cutoff. |
| P1-81 | #11 | threshold-miss | ISTFTHead forward/forward_chunk verbatim block under cutoff. |
| P1-82 | #25 | detector-miss | #25 silent; `isift` (typo of istft) rename uncaught. |
| P1-83 | none | inventory-gap | `-> Tensor` returns 2-tuple; no rule. |
| P1-84 | #11 | threshold-miss | _init_weights near-clone of the reported group, excluded by dropped bias guard. |
| P1-85 | none | inventory-gap | Commented-out safety assert; no rule. |
| P1-86 | none | inventory-gap | Commented-out alt implementation; no rule. |
| P1-87 | #1 | detector-miss | 12 non-Optional `=None` defaults; no exact rule. |
| P1-88 | none | inventory-gap | Coupled upsample literals; no rule. |
| P1-89 | #29 | covered | Module-docstring finding at decoder.py:1. |
| P1-90 | #13 | detector-miss | `WNConv1d` single forward to `weight_norm(Conv1d)`; #13 active, missed (only #1 fired here). |
| P1-91 | #1 | covered | opaque-kwargs finding at rvq.py:8. |
| P1-92 | #1 | threshold-miss | `rvq_dim=None` unannotated among annotated siblings; #1 fired at rvq.py:8, this instance under cutoff. |
| P1-93 | #12 | detector-miss | Unused `enumerate` index; #12 silent repo-wide. |
| P1-94 | #28 | detector-miss | Comment documents a 4-value return vs 2 actual; #28 doesn't check comment prose. |
| P1-95 | #2 | detector-miss | Logical (not type) redundancy `norm is not None and norm=="slaney"`; oracle can't prove it (norm is Optional). |
| P1-96 | #13 | detector-miss | `make_nonpad_mask` single-forward-with-negation; #13 active, missed. |
| P1-97 | #9 | detector-miss | Module-global `model` mutated cross-scope; #9 fired on mutable-defaults, missed the module-global half. |
| P1-98 | #9 | detector-miss | Module-global `global_lang`; #9 missed module-global half. |
| P1-99 | none | inventory-gap | No-op `global` declaration; no rule. |
| P1-100 | none | inventory-gap | `None != ""` gate bypass; no rule. |
| P1-101 | #8 | detector-miss | #8 silent; extraction-regex vs validator disagreement uncaught. |
| P1-102 | #8 | detector-miss | #8 silent; hand-unrolled four-tag disjunction uncaught. |
| P1-103 | none | inventory-gap | Unused `progress_bar` binding; no rule. |
| P1-104 | none | inventory-gap | Unused imports; no rule. |
| P1-105 | #18 | covered | section-comments finding at gradio_demo.py:204. |
| P1-106 | #1 | detector-miss | `prefix: str = None` non-Optional default; no exact rule. |
| P1-107 | none | inventory-gap | Missing install_requires; no rule. |
| P1-108 | #1 | covered | bare-dict finding at posttrain.py:22. |
| P1-109 | #30 | detector-miss | Two-level `config[..][..]` subscript coupling; #30 counts attribute Demeter, not subscripts. |
| P1-110 | none | inventory-gap | Dead `trial` param + phantom sweep; no rule. |
| P1-111 | none | inventory-gap | Unused imports; no rule. |
| P1-112 | none | inventory-gap | Conditionally-bound `writer`; no rule. |
| P1-113 | none | inventory-gap | Unused `valloader` (eager cost); no rule. |
| P1-114 | none | inventory-gap | Three disagreeing `num_workers`; no rule. |
| P1-115 | #28 | detector-miss | Tutorial documents config keys no code reads; #28 checks path resolution, not doc-vs-code key drift. |
| P1-116 | #6 | detector-miss | `final_val_loss = train()` returns None (misleading name); #6 keys on accessor names. |
| P1-117 | #11 | threshold-miss | Four-accumulator block repeated 4x, intra-function, below clone granularity. |
| P1-118 | none | inventory-gap | Dead `BucketSampler` (55 lines); no rule. |
| P1-119 | none | inventory-gap | Stub `_summary_` docstrings; no rule. |
| P1-120 | #18 | covered | section-comments finding at dataloader.py:106. |
| P1-121 | none | inventory-gap | Break-without-slice logic bug; no rule. |
| P1-122 | none | inventory-gap | Off-by-one measure/action mismatch; no rule. |
| P1-123 | #15 | detector-miss | Dead `device` param threaded through; #15 (wallet) fired once, this dead-param variant missed. |
| P1-124 | none | inventory-gap | `num_workers` param silently dropped; no rule. |
| P1-125 | #11 | threshold-miss | `get_index` vs `recovery_debug` cross-file clone under cutoff. |
| P1-126 | #12 | detector-miss | `range(len(...))` over parallel lists x3; #12 silent repo-wide. |
| P1-127 | none | inventory-gap | `TOTAL_MAX_LEN` restates inference budget; no rule. |
| P1-128 | none | inventory-gap | Unused imports; no rule. |
| P1-129 | none | inventory-gap | Unused imports; no rule. |
| P1-130 | none | inventory-gap | `open()` without context manager; no rule. |
| P1-131 | none | inventory-gap | Unvalidated hardcoded speaker tag; no rule. |
| P1-132 | none | inventory-gap | 15 unused imports; no rule. |
| P1-133 | #9 | detector-miss | Import-time global mutation (`filterwarnings`/`set_num_threads`); outside #9's module-mutable scope. |
| P1-134 | none | inventory-gap | Dead `split_list_into_chunks`; no rule. |
| P1-135 | #12 | detector-miss | `jsonl[:-6]+...` reimplements splitext; #12 silent repo-wide. |
| P1-136 | none | inventory-gap | Hardcoded `cuda` no fallback; no rule. |
| P1-137 | none | inventory-gap | `.squeeze()` rank bug; no rule. |
| P1-138 | none | inventory-gap | Dead `is_useful` flag (filter never applied); no rule. |
| P1-139 | #15 | detector-miss | `get_speaker_dict` built for a `len()` (demand narrowing); #15 active, missed this instance. |
| P1-140 | none | inventory-gap | Dead `recovery_debug`/`read_test` (85 lines); no rule. |
| P1-141 | none | inventory-gap | Dead `idx = 1` store; no rule. |
| P1-142 | none | inventory-gap | Hardcoded `16` (3rd home); no rule. |
| P1-143 | #11 | threshold-miss | Prompt-template string duplicated train/inference cross-file; under clone cutoff. |
| P1-144 | #8 | detector-miss | Speaker cap `>5` vs 4-tag rule elsewhere; #8 silent repo-wide. |
| P1-145 | none | inventory-gap | Inline filter thresholds beside named constants; no rule. |
| P1-146 | none | inventory-gap | Two length columns in different units; no rule. |
| P1-147 | none | inventory-gap | 9 unused imports; no rule. |
| P1-148 | none | inventory-gap | Chinese-only comments; no rule. |
