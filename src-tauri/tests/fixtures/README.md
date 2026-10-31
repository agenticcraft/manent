# Engine test fixtures

Read speech from Google FLEURS (https://huggingface.co/datasets/google/fleurs), CC-BY-4.0.
16 kHz mono 16-bit WAV, cut from the VRT-2/VRT-32 spike samples.

| File | Content |
|---|---|
| `nl-fleurs-1784.wav` | nl_nl test row 1784: "groenland had nog weinig inwoners volgens noorse sagen werd erik de rode uit ijsland verbannen wegens moord vond groenland op weg naar het westen en gaf het zijn naam" |
| `en-fleurs-1652.wav` | en_us validation row 1652: "the major religion in moldova is orthodox christian" |
| `en-two-speakers.wav` | en_us validation rows 1652, 1594, 1531, 1533, alternating a lower and a higher voice; turns in `en-two-speakers.truth.json` |

`import/tone.*`: a generated 2 s, 440 Hz stereo tone (ffmpeg `sine`) in wav, mp3, m4a (AAC), mp4 (H.264 + AAC) and ogg (Vorbis), for the import decoder (VRT-19).
