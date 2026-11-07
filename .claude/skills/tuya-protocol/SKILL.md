---
name: tuya-protocol
description: Use when encoding or decoding DP 61 segment paint commands, scheduling sends to the strip, using rustuya, or porting the Smart Life QR cloud login. Format, fixtures, measured limits.
---

# Tuya protocol

Spec §5.2-§5.5 (verified in the 2026-11-06 spike). Pure encoding and scheduling belong in `light-core`;
I/O in `src-tauri`. Pair with the `security` skill for anything network or secret related.

## Device and local connection
- Reference strip: JSK-P22, category `dd`, local protocol 3.3, 10 segments (9 visible pieces).
  It was Bluetooth-only until re-added in Smart Life on 2.4 GHz wifi; that changed its ID and key.
- Library `rustuya` 0.4.x (MIT, feature `tokio`): `Discovery` finds ID, IP, version, `productKey`;
  `Device::builder(id, key).address(ip).version(V3_3).connect()`; `set_value(dp, value)`; `listener()`.
- The cloud does not advertise the segment DP (only `switch_led`, `work_mode`, `colour_data`, `countdown`) but the strip uses it locally.
- `raw` DPs travel as base64 inside the JSON.

## DP 61 (`paint_colour_data`, raw)
```
Header:  00  MM  00  NN      version 0x00 | mode (0x01 colour; Smart Life uses 0x02, both work)
                             | effect 0x00 | NN = segment count (0x0a = 10)
Command: AA  HHHH SSSS VVVV  [SEL seg...]
         AA: 0x00 bucket (whole strip, no SEL) | 0x01 pencil | 0x02 eraser (HSV zero)
         H 0-360, S 0-1000, V 0-1000, 16-bit big-endian
         SEL = 0x80 | k, then k segment numbers (1-based)
```
White is `S=0, V=1000`.

## Fixtures (fixed test data, hex without spaces in tests)
| Action | Hex |
|---|---|
| Red bucket | `0001000a 00 0000 03e8 03e8` |
| Blue bucket | `0001000a 00 00f0 03e8 03e8` |
| Red pencil on 1,3,5,7,9 | `0001000a 01 0000 03e8 03e8 85 0103050709` |
| White pencil on 2,4,6,8,10 | `0001000a 01 0000 0000 03e8 85 020406080a` |
| Eraser on segment 8 (Smart Life) | `0002000a 02 0000 0000 0000 81 08` |

## Measured limits
- Only the FIRST command of each message applies; a second concatenated one is ignored.
  A scene with N distinct colours needs N messages.
- One message per change held up at 2, 4 and 8 changes/s (160 of 162 at 8/s).
- Two back-to-back messages per change at 2/s lost some and showed intermediate states and wrong order: space messages out.
- The strip acks in ~100 ms and reports new state in ~400 ms; when the change becomes visible is NOT measured.
- ICMP ping takes 1-2 s (wifi power saving); TCP is unaffected.
- N comes from the header byte seen when the user paints in Smart Life (unverified assumption: the strip does not announce N).

## Smart Life QR login (port of `tuya-device-sharing-sdk` 0.2.15, MIT)
1. `POST https://apigw.iotbing.com/v1.0/m/life/home-assistant/qrcode/tokens?clientid=HA_3y9q4ak7g4ephrvke&usercode=<code>&schema=haauthorize`
   gives `result.qrcode` (the token).
2. Show a locally generated QR with text `tuyaSmart--qrLogin?token=<token>`; the user scans and confirms in Smart Life.
3. Poll `GET .../qrcode/tokens/<token>?clientid=...&usercode=...` (on a timer, with a deadline) until `success`:
   `access_token`, `refresh_token`, `uid`, `expire_time`, `terminal_id`, `endpoint`.
4. With that session: `GET /v1.0/m/life/users/homes`, then `GET /v1.0/m/life/ha/home/devices?homeId=...`
   (`id`, `name`, `category`, `product_name`, `local_key`, `online`). These requests are AES-GCM encrypted and
   HMAC-SHA256 signed (key derived from a `requestId` and the `refresh_token`); port `_secret_generating`,
   `_restful_sign`, `_aes_gcm_encrypt`, `_aex_gcm_decrypt` from `tuya_sharing/customerapi.py` as they are.
5. Revoke the session (`POST /v1.0/m/token/terminal/expire`) and drop it from memory.

The Python flow worked twice with the real account; the Rust port is not proven yet. If it does not work, stop and ask.
Accepted risk: it uses Home Assistant's app registration (`client_id`); Tuya could cut it off (plan B: manual key, phase 2).
