# Current focus (2026-07-28)

**Product rule:** all capabilities are **first-party in-repo**. No plugin marketplace, no runtime third-party plugin host. Optional enhancements stay in main binary or are skipped—not external packages.

Stabilization + feature wiring on fixed façades. **Do not** change OCR session lifecycle (`ocr_begin/end_session_*`, selector→result baton).

## Done recently

| Slice | Where |
|-------|--------|
| OCR session lifecycle (main out of OCR) | `ocr_begin/end_session_*`, `OcrScreenshotTranslator` |
| Feature sprint Wave A/B | collection push-on-save, control/bridge UX, tray label, numbered LLM parse, OCR UI feedback, post-process settings, engine settings sections |
| CI | pnpm packageManager, archive stale demos, TS EnginesConfig fix |
| Repo | strip Claude co-author trailers; plugin marketplace **removed** |

## Priority order

1. Keep façades modular (engine / collection / control / OCR present).
2. Manual OCR smoke when free — [OCR_SMOKE.md](./OCR_SMOKE.md) (not blocking feature wiring).
3. Optional Wave C (in-app only): ExternalCall mutex, overlay DPI, layout analyzer **default off** — see plan notes.
4. Extension depth (batch/context) after desktop stable.
5. Hook: passive only; DLL inject stays experimental / no deep invest.

## Non-goals

- Plugin marketplace, plugin SDK, sandbox HTTP plugin engines  
- Luna inject deep-dive, multi-end Cloudflare, FSRS product expansion, AppState big-bang  
- Floating multi-engine popup (new window / z-order risk) unless explicitly reopened  

## Architecture

```
UI / tray / extension / HTTP control  →  commands / api_server
  → translate · collection · capture · post_process
  → engine/* · collection/* · Win32
```

New features = first-party module + one command/settings key + CI green.

## References

- [MODULE_MAP.md](./MODULE_MAP.md) · [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) · [FEATURES.md](./FEATURES.md)
