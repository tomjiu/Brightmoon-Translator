# Localization Guide

Moon Translator supports multiple languages. This guide explains how to add new translations or improve existing ones.

## Supported Languages

| Language | Code | File |
|----------|------|------|
| Chinese (Simplified) | `zh` | `src/i18n/zh.json` |
| English | `en` | `src/i18n/en.json` |
| Japanese | `ja` | `src/i18n/ja.json` |
| Korean | `ko` | `src/i18n/ko.json` |

## Adding a New Language

### Step 1: Create Translation File

1. Copy `src/i18n/en.json` as a template
2. Rename it to `<language-code>.json` (e.g., `fr.json` for French)
3. Translate all values in the file

### Step 2: Register the Language

Edit `src/i18n/index.ts`:

```typescript
import { create } from "zustand";
import zh from "./zh.json";
import en from "./en.json";
import ja from "./ja.json";
import ko from "./ko.json";
import fr from "./fr.json";  // Add import

export type Locale = "zh" | "en" | "ja" | "ko" | "fr";  // Add to type

const locales: Record<Locale, Record<string, unknown>> = { zh, en, ja, ko, fr };  // Add to map
```

### Step 3: Add UI Selector

Edit `src/pages/Settings.tsx` and add a button in the language selector section:

```tsx
<button
  className={`px-4 py-2 rounded-lg text-sm border transition-colors ${
    locale === "fr"
      ? "bg-primary text-white border-primary"
      : "bg-bg-tertiary text-text-secondary border-border hover:border-primary"
  }`}
  onClick={() => setLocale("fr")}
>
  Français
</button>
```

## Translation Structure

The translation file uses nested JSON with dot notation keys:

```json
{
  "section": {
    "subsection": {
      "key": "Translated text"
    }
  }
}
```

Access in code: `t("section.subsection.key")`

## Translation Keys

### Common Sections

- `app` - Application name
- `nav` - Navigation menu
- `translator` - Main translator interface
- `history` - Translation history
- `settings` - Settings pages
- `tools` - Toolbox features
- `wordbook` - Word book
- `documents` - Document viewers (PDF, EPUB, Subtitle)
- `plugins` - Plugin management
- `ocr` - OCR monitoring
- `hook` - Hook immersive translation
- `compare` - Multi-engine comparison
- `batch` - Batch translation
- `tm` - Translation memory
- `common` - Common UI elements

### Parameterized Strings

Some strings use parameters with `{param}` syntax:

```json
{
  "totalRecords": "{count} records total"
}
```

In code: `t("history.totalRecords", { count: 42 })`

## Translation Guidelines

1. **Keep keys consistent** - All language files must have the same keys
2. **Preserve parameters** - Keep `{param}` placeholders intact
3. **Match context** - Understand where the string appears in UI
4. **Be concise** - UI space is limited
5. **Use natural language** - Avoid literal translations

## Quality Checklist

Before submitting translations:

- [ ] All keys from `en.json` are present
- [ ] No missing translations (empty strings)
- [ ] Parameters are preserved
- [ ] Text fits UI constraints
- [ ] Grammar and spelling are correct
- [ ] Technical terms are translated appropriately

## Testing Translations

1. Run `npm run dev` to start development server
2. Go to Settings > Language
3. Select your language
4. Verify all UI elements display correctly

## Contributing

1. Fork the repository
2. Create a branch for your translations
3. Add/update translation files
4. Test thoroughly
5. Submit a pull request

## Reporting Issues

If you find translation issues:

1. Open an issue with the language and location
2. Provide the correct translation
3. Include screenshots if helpful
