# Hover Translation Test Checklist

## Test Environment
- Extension version: 1.0.0
- Browser: Chrome / Edge
- Desktop bridge: test with and without desktop app running

---

## 1. Plain Article Page (e.g., Wikipedia, news site)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over paragraph | Tooltip appears after 300ms | |
| Translation result | Correct translation shown | |
| Move to different paragraph | Tooltip updates | |
| Move mouse away | Tooltip hides within 200ms | |
| Scroll page | Tooltip hides immediately | |
| Hover over link inside text | Skipped (link is in SKIP_TAGS) | |
| Bridge path | Desktop bridge used if available | |
| Fallback path | Local engine used if desktop off | |

## 2. Documentation Page (e.g., MDN, React docs)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over heading | Translates heading text | |
| Hover over code block | Skipped (CODE/PRE tags) | |
| Hover over inline code | Skipped | |
| Hover over list item | Translates list item | |
| Hover over table cell | Translates cell content | |
| Tooltip doesn't overlap header | Position adjusts | |

## 3. Search Results Page (e.g., Google, Bing)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over result title | Skipped (inside A tag) | |
| Hover over result snippet | Translates snippet | |
| Hover over "People also ask" | Translates question text | |
| Rapid hover between results | No rapid-fire requests (debounce) | |

## 4. Link-Heavy Page (e.g., Reddit, Hacker News)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over link text | Skipped (A tag) | |
| Hover over comment body | Translates comment | |
| Hover over nested elements | Walks up to find text block | |
| No hover loop on tooltip | Tooltip mouseenter stops hide | |

## 5. Code Block Page (e.g., GitHub README, Stack Overflow)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over code block | Skipped | |
| Hover over surrounding text | Translates | |
| Hover over diff view | Skipped (PRE tag) | |
| Hover over answer text | Translates | |

## 6. SPA Page (e.g., React app, Vue app)

| Check | Expected | Result |
|-------|----------|--------|
| Hover over dynamic content | Translates after delay | |
| Content updates while hovering | Tooltip shows old result (no re-trigger) | |
| Navigate to different view | Tooltip hides on navigation | |
| Hover over modal overlay | Translates modal text | |

## Settings Tests

| Check | Expected | Result |
|-------|----------|--------|
| Disable hover in popup | Hover stops working immediately | |
| Re-enable hover | Hover works again | |
| Change delay to 1000ms | Noticeably slower trigger | |
| Change min length to 20 | Short text ignored | |
| Set modifier key to Alt | Only triggers with Alt held | |
| Release Alt key | Tooltip hides | |
| Set modifier key to None | Works without modifier | |

## Compatibility Tests

| Check | Expected | Result |
|-------|----------|--------|
| contenteditable div | Skipped | |
| Textarea | Skipped | |
| Button with text | Skipped | |
| Select dropdown | Skipped | |
| SVG element | Skipped | |
| Hidden element (display:none) | No text extracted | |
| Element with tabindex | Skipped (interactive) | |
| Role="textbox" element | Skipped | |

## Edge Cases

| Check | Expected | Result |
|-------|----------|--------|
| Very long text (>2000 chars) | Ignored | |
| Very short text (1 char) | Ignored | |
| Empty text | Ignored | |
| Page with no text | No tooltip | |
| Multiple rapid hovers | Only last hover triggers | |
| Click while tooltip shown | Click passes through | |
| Escape key | Tooltip hides | |
| Dark mode page | Tooltip readable | |

---

## Summary

- Total checks: 42
- Passed: __
- Failed: __
- Notes:
