# PolySynth

Rust製のポリフォニック・バーチャルアナログシンセサイザー(CLAPプラグイン)。

クロスプラットフォーム(Linux / macOS / Windows)。[nice-plug](https://codeberg.org/RustAudio/nice-plug)
(nih-plug の公式後継フォーク)上に構築。

## シグナルフロー

```
OSC1 (PolyBLEP) ─┐
OSC2 (PolyBLEP) ─┼─ MIXER ─ tanh DRIVE ─ SVF LPF ─ VCA ─ OUT
NOISE (xorshift) ┘              ▲                    ▲
                     FILTER ENV + KEYTRACK       AMP ENV
                          ▲
              LFO ────────┴──(Pitch / Cutoff / Amp)
```

- **オシレータ×2**: PolyBLEP帯域制限付き saw / square / pulse(PWM可)、三角波はBLEP矩形波のリーキー積分、サイン波。OSC2はオクターブ(±2)・デチューン(±100セント)
- **フィルタ**: Simper型 TPT(ゼロ遅延フィードバック)SVF ローパス。レゾナンス、tanhドライブ、キートラッキング、エンベロープ量(±96半音)
- **エンベロープ×2**(アンプ/フィルタ): アナログ風指数カーブADSR、クリックフリーリトリガー
- **LFO**: sine / tri / saw / square / S&H → ピッチ / カットオフ / アンプ
- **ポリフォニー**: 最大16ボイス(可変)、リリース優先の最古ボイススチール、CLAPポリフォニックモジュレーション対応(per-voice gain)
- **GUI**: egui製カスタムエディタ(自作ノブウィジェット、Shiftで微調整、ダブルクリックでリセット)

## ビルド

要件: Rust stable ≥ 1.87

```sh
# CLAPバンドルの生成 → target/bundled/PolySynth.clap
cargo xtask bundle polysynth --release
```

生成された `PolySynth.clap` をCLAP対応DAW(Bitwig Studio, REAPER 等)のプラグイン
ディレクトリに配置してください。

### スタンドアロン実行(JACK/ALSA)

```sh
cargo run -p polysynth --features standalone --release
```

## テスト

```sh
cargo test          # DSPユニットテスト(エイリアシングFFT、フィルタ特性、ENVタイミング、ボイス管理)
cargo clippy --all-targets
```

CLAP適合性は [clap-validator](https://github.com/free-audio/clap-validator) で検証:

```sh
clap-validator validate target/bundled/PolySynth.clap
```

## 構成

```
polysynth/src/
├── lib.rs              # プラグインシェル、サブブロック処理ループ、CLAPエクスポート
├── params.rs           # 全パラメータ定義(スムージング・スキュー・フォーマッタ)
├── voice_manager.rs    # 16ボイス固定プール、スチール、VoiceTerminated
├── dsp/
│   ├── oscillator.rs   # PolyBLEPオシレータ
│   ├── noise.rs        # xorshiftホワイトノイズ
│   ├── filter.rs       # TPT SVF
│   ├── envelope.rs     # 指数ADSR
│   ├── lfo.rs          # LFO
│   └── voice.rs        # ボイス信号チェーン
└── editor/             # eguiエディタ(knob.rs = 自作ノブ)
```

## ライセンス

ISC(nice-plug に準拠)。VST3ビルドは意図的に無効化しています(VST3バインディングはGPLv3のため)。
