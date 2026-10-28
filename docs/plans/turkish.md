# Turkish interface language

Status: built on `feat/turkish` (catalog and every client registration); not merged yet.

## Goal

Turkish becomes the ninth selectable interface language, next to English, Simplified and
Traditional Chinese, Japanese, Korean, French, German and Spanish (added in 3b612a6d). With the
language left on "Follow system", a Mac or phone set to Turkish shows Codync in Turkish. Bot names,
conversation content, agent output, code, paths and model names are never translated.

The owner asked for the desktop app; the catalog is shared, so the iPhone, Android and terminal
apps register the language in the same change at little extra cost (the repo's cross-client rule).

## Order

1. Prerequisite: `feat/roster-sections` (PR #202) is merged with `upstream/dev` (2.31.2), bumped to
   2.32.0 with every client's section gate raised to 2.32.0, its section text added to every
   catalog, and the automated review's findings fixed.
2. `feat/turkish` branches from it, so the section text is translated too.

## Design

- Catalog: `apps/ios/Kit/Sources/CodyncKit/Resources/Languages/tr.json`, the same keys as
  `zh-Hans.json` (the English phrase is the key, `{token}` slots kept), every value Turkish.
  `"English": "English"` stays as in the other catalogs; the language's own name is `Türkçe`.
- Each client adds `tr` exactly where 3b612a6d added `ja`/`ko`/`fr`/`de`/`es`:
  - desktop `apps/desktop/src/renderer/lib/l10n.ts` (`appLanguages`, `catalogs`, native name)
    and `catalogs.test.ts`;
  - iOS `apps/ios/Kit/Sources/CodyncKit/L10n.swift` (`AppLanguage`, catalogs, locale, menu
    title);
  - Android `apps/android/design/src/main/kotlin/com/codync/android/design/L10n.kt`
    (`AppLanguage`);
  - terminal `host/src/tui/l10n.rs`.
- "Follow system" picks Turkish when the system language starts with `tr`.
- The settings menus list `appLanguages` (desktop) and `AppLanguage.allCases` (iOS), so they need
  no edit.

## Wording

Informal "sen" address, with the glossary in the
[localization guide](../guides/localization.md#turkish-wording).

## Testing

- `catalogs.test.ts` includes `tr`: same keys as the other catalogs, every value non-empty, the
  same `{token}` slots.
- Each client's language tests cover `tr` the way they cover the other languages (stored value,
  "Follow system" with a `tr-TR` system language).
- End to end: the desktop dev app with the Mac in Turkish (`tr-TR`): roster with sections, a
  conversation, settings (language menu shows Türkçe), routines, usage, Extensions, the menu bar and
  tray menus; look for clipped or overflowing text and leftover English. Then package the app for
  the owner's Mac.
