# Compatibility Test Matrix

## Test Environment
- OS: Windows 11
- moontranslator version: [version]
- Date: [date]

## Applications to Test

### Selection Success

| App | Type | UIA | Clipboard | Provider Used | Confidence | Notes |
|-----|------|-----|-----------|---------------|------------|-------|
| Notepad | Native | | | | | |
| VS Code | Electron | | | | | |
| Chrome | Browser | | | | | |
| Firefox | Browser | | | | | |
| Edge | Browser | | | | | |
| Word | Office | | | | | |
| Excel | Office | | | | | |
| Outlook | WebView2 | | | | | |
| Teams | WebView2 | | | | | |
| Obsidian | Electron | | | | | |
| Telegram | Electron | | | | | |
| Slack | Electron | | | | | |
| Sublime Text | Native | | | | | |
| Notepad++ | Native | | | | | |
| Adobe Reader | Native | | | | | |
| SumatraPDF | Native | | | | | |

### Replace Success

| App | Type | Success | Failure Type | Fallback | Notes |
|-----|------|---------|-------------|----------|-------|
| Notepad | Native | | | | |
| VS Code | Electron | | | | |
| Chrome | Browser | | | | |
| Firefox | Browser | | | | |
| Edge | Browser | | | | |
| Word | Office | | | | |
| Excel | Office | | | | |
| Outlook | WebView2 | | | | |
| Teams | WebView2 | | | | |
| Obsidian | Electron | | | | |
| Telegram | Electron | | | | |
| Slack | Electron | | | | |
| Sublime Text | Native | | | | |
| Notepad++ | Native | | | | |
| Adobe Reader | Native | | | | |
| SumatraPDF | Native | | | | |

### Read-Only Detection

| App | Control | Readonly Detected | Overlay Fallback | Notes |
|-----|---------|-------------------|------------------|-------|
| Chrome | Address bar | | | |
| Chrome | Read-only input | | | |
| VS Code | Terminal | | | |
| Outlook | Email body (received) | | | |
| Adobe Reader | PDF viewer | | | |
| Word | Protected document | | | |

### OCR Usability

| App | Region Capture | Text Quality | Translation | Notes |
|-----|---------------|-------------|-------------|-------|
| Chrome | | | | |
| Firefox | | | | |
| Edge | | | | |
| Word | | | | |
| PDF Viewer | | | | |
| Image viewer | | | | |
| Video player | | | | |

### Overlay Follow Quality

| App | Mode | Tracks Window | Jitter | Notes |
|-----|------|--------------|--------|-------|
| Notepad | TargetBounds | | | |
| VS Code | TargetBounds | | | |
| Chrome | TargetBounds | | | |
| Word | TargetBounds | | | |
| Notepad | Cursor | | | |
| VS Code | Cursor | | | |

### Confidence → Overlay Level

| Confidence | Expected Level | Actual Level | Correct | Notes |
|-----------|---------------|-------------|---------|-------|
| >= 0.90 | User configured | | | |
| 0.70-0.89 | Standard (L2) | | | |
| < 0.70 | Full (L3) | | | |

### TextPattern2 Cascade

| App | TextPattern | TextPattern2 | GetCaretRange | Notes |
|-----|------------|-------------|---------------|-------|
| Chrome | | | | |
| Firefox | | | | |
| Edge | | | | |
| VS Code | | | | |
| Word | | | | |

### Browser Extension

| Site | Hover Translation | Page Translation | Cache Hit | Glossary | Notes |
|------|------------------|-----------------|-----------|----------|-------|
| Wikipedia | | | | | |
| GitHub | | | | | |
| Stack Overflow | | | | | |
| News site | | | | | |
| SPA app | | | | | |

## Scoring Legend

### Selection
- **UIA**: Success via UI Automation
- **Clipboard**: Success via Ctrl+C simulation
- **Provider Used**: Which provider succeeded
- **Confidence**: 0.0-1.0 score from UIA

### Replace
- **Success**: Replace worked
- **Failure Type**: ClipboardOpenFailed / ClipboardWriteFailed / Read-Only / Other
- **Fallback**: Overlay shown as fallback

### OCR
- **Region Capture**: Can select region
- **Text Quality**: OCR text accuracy
- **Translation**: Translation works

### Overlay Follow
- **Tracks Window**: Follows window movement
- **Jitter**: None / Slight / Heavy

### Browser Extension
- **Hover Translation**: Works on hover
- **Page Translation**: Full page translate works
- **Cache Hit**: Second translation hits cache
- **Glossary**: Glossary terms applied correctly

## Test Procedure

1. **Selection Test**: Select text in each app, trigger translation
2. **Replace Test**: After selection, check if text is replaced in-place
3. **Read-Only Test**: Try replace on known read-only controls
4. **OCR Test**: Use OCR on each app window
5. **Overlay Test**: Move/resize target window, check overlay tracking
6. **Extension Test**: Use browser extension on various sites
7. **Confidence Test**: Verify overlay level matches confidence thresholds
8. **TextPattern2 Test**: Check logs for TextPattern2 usage

## Known Issues

Document any known issues or workarounds discovered during testing.
